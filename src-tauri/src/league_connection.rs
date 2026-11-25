//! League's local HTTPS connection is independent of whether its window can be placed.
use crate::{league_path, procs};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    fs,
    os::windows::io::{FromRawHandle, OwnedHandle},
    sync::mpsc::SyncSender,
    time::Duration,
};
use tokio::{
    sync::watch,
    time::{self, Instant},
};
use tokio_tungstenite::{
    tungstenite::{
        handshake::{client::generate_key, derive_accept_key},
        protocol::{Role, WebSocketConfig},
        Message,
    },
    WebSocketStream,
};
use windows_sys::{
    Wdk::System::Threading::NtQueryInformationProcess,
    Win32::{
        Foundation::UNICODE_STRING,
        System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    },
};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Connection {
    pid: u32,
    port: u16,
    password: String,
}

pub(crate) fn publish(sender: &watch::Sender<Option<Connection>>, next: Option<Connection>) {
    sender.send_if_modified(|current| {
        if *current == next {
            false
        } else {
            *current = next;
            true
        }
    });
}

#[test]
fn unchanged_discovery_keeps_the_event_connection_open() {
    let (sender, receiver) = watch::channel(None);
    publish(&sender, None);
    assert!(!receiver.has_changed().unwrap());
    let connection = Connection::new(1, "12345", "test-only").unwrap();
    publish(&sender, Some(connection.clone()));
    let mut receiver = receiver;
    assert!(receiver.has_changed().unwrap());
    receiver.borrow_and_update();
    publish(&sender, Some(connection));
    assert!(!receiver.has_changed().unwrap());
    publish(&sender, None);
    assert!(receiver.has_changed().unwrap());
}

impl Connection {
    fn new(pid: u32, port: &str, password: &str) -> Option<Self> {
        let port = port.parse::<u16>().ok().filter(|port| *port > 0)?;
        if pid == 0 || password.is_empty() {
            return None;
        }
        Some(Self {
            pid,
            port,
            password: password.into(),
        })
    }

    fn lock(text: &str) -> Option<Self> {
        let parts: Vec<_> = text.trim().split(':').collect();
        let [name, pid, port, password, "https"] = parts.as_slice() else {
            return None;
        };
        if !name.eq_ignore_ascii_case("LeagueClient")
            && !name.eq_ignore_ascii_case("LeagueClientUx")
        {
            return None;
        }
        Self::new(pid.parse().ok()?, port, password)
    }

    fn from_command_line(pid: u32, command_line: &str) -> Option<Self> {
        let mut port = None;
        let mut password = None;
        for argument in command_line.split_whitespace() {
            let Some((name, value)) = argument.trim_matches('"').split_once('=') else {
                continue;
            };
            if name.eq_ignore_ascii_case("--app-port") {
                port = Some(value);
            } else if name.eq_ignore_ascii_case("--remoting-auth-token") {
                password = Some(value);
            }
        }
        let password = password?;
        if !password
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return None;
        }
        Self::new(pid, port?, password)
    }

    /// Prefer the newest lock belonging to a running League client. Some installs omit the
    /// lock or leave an old copy, so fall back to the arguments of a running LeagueClientUx.
    pub(crate) fn find(exclude: Option<&Self>) -> Option<Self> {
        let processes = procs::list().ok()?;
        let clients: Vec<_> = processes
            .iter()
            .filter(|row| league_path::is_client(&row.name))
            .collect();
        if clients.is_empty() {
            return None;
        }
        let locks = league_path::folders_for(&processes)
            .into_iter()
            .filter_map(|folder| {
                let lock = folder.join("lockfile");
                let meta = fs::metadata(&lock).ok()?;
                if !meta.is_file() || meta.len() > 4096 {
                    return None;
                }
                let candidate = Self::lock(&fs::read_to_string(&lock).ok()?)?;
                (exclude != Some(&candidate) && clients.iter().any(|row| row.pid == candidate.pid))
                    .then(|| (meta.modified().ok(), candidate))
            });
        if let Some((_, candidate)) = locks.max_by_key(|(modified, _)| *modified) {
            return Some(candidate);
        }
        clients
            .into_iter()
            .filter(|row| row.name == "leagueclientux.exe")
            .filter_map(|row| Self::from_command_line(row.pid, &command_line(row.pid)?))
            .find(|candidate| exclude != Some(candidate))
    }

    pub(crate) fn http(timeout: Duration) -> Result<reqwest::blocking::Client, reqwest::Error> {
        reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .danger_accept_invalid_certs(true)
            .timeout(timeout)
            .build()
    }

    pub(crate) fn get(&self, http: &reqwest::blocking::Client, endpoint: &str) -> Option<Value> {
        let response = self
            .request(http, reqwest::Method::GET, endpoint)
            .send()
            .ok()?;
        crate::guide::capped_json(response, 4 * 1024 * 1024).ok()
    }

    pub(crate) fn request(
        &self,
        http: &reqwest::blocking::Client,
        method: reqwest::Method,
        endpoint: &str,
    ) -> reqwest::blocking::RequestBuilder {
        // LCU uses a self-signed certificate. Credentials stay on IPv4 loopback with no proxy
        // and no redirects. Callers use fixed endpoints and never log the credentials.
        http.request(method, format!("https://127.0.0.1:{}{endpoint}", self.port))
            .basic_auth("riot", Some(&self.password))
    }
}

const TOPICS: [&str; 4] = [
    "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase",
    "OnJsonApiEvent_lol-champ-select_v1_session",
    "OnJsonApiEvent_lol-lobby_v2_lobby",
    "OnJsonApiEvent_lol-lobby_v2_lobby_matchmaking_search-state",
];

fn phase_event(text: &str) -> bool {
    let Ok(message) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    message[0] == 8
        && message[1]
            .as_str()
            .is_some_and(|topic| TOPICS.contains(&topic))
}

fn wake(signal: &SyncSender<()>) {
    let _ = signal.try_send(());
}

/// Events wake the HTTP poll immediately. Polling still works when League's event socket is down.
pub(crate) async fn events(
    mut connection: watch::Receiver<Option<Connection>>,
    signal: SyncSender<()>,
) {
    let Ok(http) = reqwest::Client::builder()
        .http1_only()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(3))
        .build()
    else {
        return;
    };
    loop {
        let current = connection.borrow_and_update().clone();
        if let Some(current) = current {
            tokio::select! {
                result = connection.changed() => {
                    if result.is_err() { return; }
                    continue;
                }
                () = listen(http.get(format!("https://127.0.0.1:{}/", current.port))
                    .basic_auth("riot", Some(&current.password)), &signal) => {}
            }
            tokio::select! {
                result = connection.changed() => if result.is_err() { return; },
                () = time::sleep(Duration::from_secs(2)) => {}
            }
        } else if connection.changed().await.is_err() {
            return;
        }
    }
}

async fn listen(request: reqwest::RequestBuilder, signal: &SyncSender<()>) {
    let key = generate_key();
    let Ok(response) = request
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", &key)
        .header("Sec-WebSocket-Protocol", "wamp")
        .send()
        .await
    else {
        return;
    };
    let header = |name| {
        response
            .headers()
            .get(name)
            .and_then(|header| header.to_str().ok())
    };
    if response.status() != reqwest::StatusCode::SWITCHING_PROTOCOLS
        || !header("upgrade").is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
        || !header("connection").is_some_and(|v| {
            v.split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
        })
        || header("sec-websocket-accept") != Some(derive_accept_key(key.as_bytes()).as_str())
        || header("sec-websocket-protocol") != Some("wamp")
        || response.headers().contains_key("sec-websocket-extensions")
    {
        return;
    }
    let Ok(upgraded) = response.upgrade().await else {
        return;
    };
    let config = WebSocketConfig::default().max_message_size(Some(8 * 1024 * 1024));
    let mut socket = WebSocketStream::from_raw_socket(upgraded, Role::Client, Some(config)).await;
    for topic in TOPICS {
        if socket
            .send(Message::Text(json!([5, topic]).to_string().into()))
            .await
            .is_err()
        {
            return;
        }
    }
    wake(signal);
    let period = Duration::from_secs(20);
    let mut ping = time::interval_at(Instant::now() + period, period);
    let mut pong = None;
    loop {
        tokio::select! {
            _ = ping.tick() => {
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                    return;
                }
                pong = Some(Instant::now() + Duration::from_secs(10));
            }
            () = async {
                match pong {
                    Some(deadline) => time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => return,
            message = socket.next() => match message {
                Some(Ok(Message::Text(text))) if phase_event(text.as_str()) => wake(signal),
                Some(Ok(Message::Pong(_))) => pong = None,
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return,
                _ => {}
            },
        }
    }
}

fn command_line(pid: u32) -> Option<String> {
    const PROCESS_COMMAND_LINE: i32 = 60;
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let _handle = unsafe { OwnedHandle::from_raw_handle(handle) };
    let mut size = 4096_u32;
    // The buffer starts with a UNICODE_STRING followed by UTF-16 text. Keep it pointer-aligned
    // and use offsets into our buffer after validating the span Windows returned.
    for _ in 0..2 {
        let mut buffer = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        let mut needed = 0;
        let status = unsafe {
            NtQueryInformationProcess(
                handle,
                PROCESS_COMMAND_LINE,
                buffer.as_mut_ptr().cast(),
                size,
                &mut needed,
            )
        };
        if status == 0 {
            let string = unsafe { buffer.as_ptr().cast::<UNICODE_STRING>().read() };
            let offset = (string.Buffer as usize).checked_sub(buffer.as_ptr() as usize)?;
            let length = usize::from(string.Length);
            if offset < size_of::<UNICODE_STRING>()
                || offset % 2 != 0
                || length % 2 != 0
                || offset.checked_add(length)? > size as usize
            {
                return None;
            }
            let text = unsafe {
                std::slice::from_raw_parts(
                    buffer.as_ptr().cast::<u16>().add(offset / 2),
                    length / 2,
                )
            };
            return String::from_utf16(text).ok();
        }
        if needed <= size || needed > 256 * 1024 {
            return None;
        }
        size = needed;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regional_client_arguments_are_a_fallback_for_a_missing_lock() {
        let lock = Connection::lock("LeagueClient:42:2999:test_Token-2:https\n").unwrap();
        let parsed = Connection::from_command_line(
            42,
            r#""D:\Riot Games\LeagueClientUx.exe" "--app-port=2999" "--remoting-auth-token=test_Token-2""#,
        );
        assert!(parsed.as_ref() == Some(&lock));
        assert!(Connection::lock("Other:42:2999:example:https").is_none());
        assert!(Connection::lock("LeagueClient:42:2999:example:http").is_none());
        assert!(Connection::lock("LeagueClient:42:0:example:https").is_none());
        assert!(Connection::from_command_line(42, "--app-port=2999").is_none());
    }

    #[test]
    fn windows_returns_the_current_process_command_line() {
        let args = command_line(std::process::id()).expect("the current process can be queried");
        let exe = std::env::current_exe().unwrap();
        assert!(args.contains(exe.file_name().unwrap().to_str().unwrap()));
    }

    #[test]
    fn only_subscribed_league_events_wake_the_poll() {
        assert!(phase_event(
            r#"[8,"OnJsonApiEvent_lol-gameflow_v1_gameflow-phase",{"data":"Lobby"}]"#
        ));
        assert!(!phase_event(
            r#"[5,"OnJsonApiEvent_lol-gameflow_v1_gameflow-phase"]"#
        ));
        assert!(!phase_event(r#"[8,"unrelated",{}]"#));
        assert!(!phase_event("invalid"));
    }

    #[tokio::test]
    async fn the_loopback_socket_subscribes_and_signals_a_draft_update() {
        use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};

        #[allow(clippy::result_large_err)] // tungstenite fixes the handshake error type.
        fn handshake(request: &Request, mut response: Response) -> Result<Response, ErrorResponse> {
            assert_eq!(request.headers()["sec-websocket-protocol"], "wamp");
            response
                .headers_mut()
                .insert("sec-websocket-protocol", "wamp".parse().unwrap());
            Ok(response)
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_hdr_async(stream, handshake)
                .await
                .unwrap();
            for topic in TOPICS {
                let message = socket.next().await.unwrap().unwrap().into_text().unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&message).unwrap(),
                    json!([5, topic])
                );
            }
            socket
                .send(Message::Text(
                    json!([8, TOPICS[1], {"data":{}}]).to_string().into(),
                ))
                .await
                .unwrap();
            socket.close(None).await.unwrap();
        };
        let http = reqwest::Client::builder()
            .no_proxy()
            .http1_only()
            .build()
            .unwrap();
        let (send, receive) = std::sync::mpsc::sync_channel(2);
        time::timeout(Duration::from_secs(4), async {
            tokio::join!(
                server,
                listen(http.get(format!("http://{address}/")), &send)
            );
        })
        .await
        .unwrap();
        assert_eq!(
            receive.try_iter().count(),
            2,
            "one wake for connecting and one for the event"
        );
    }
}
