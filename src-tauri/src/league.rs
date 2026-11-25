//! Read-only match observation. Only fixed GET endpoints are used.
use crate::{league_connection::Connection, shell::Desktop};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

fn number(value: &Value, field: &str, object: &str) -> Option<u64> {
    match value {
        Value::Object(m) => m
            .get(field)
            .and_then(Value::as_u64)
            .or_else(|| m.get(object)?.get("id")?.as_u64())
            .or_else(|| m.values().find_map(|v| number(v, field, object))),
        Value::Array(a) => a.iter().find_map(|v| number(v, field, object)),
        _ => None,
    }
}
fn mode(session: &Value, previous: Option<&str>) -> Option<String> {
    let queue = number(session, "queueId", "queue");
    if let Some(q) = queue {
        if let Some(mode) = crate::match_guide::mode(q) {
            return Some(mode.into());
        }
    }
    if session["isCustomGame"] == true || session["gameData"]["isCustomGame"] == true {
        return match number(session, "mapId", "map") {
            Some(12) => Some("aram".into()),
            Some(1 | 11) => Some("rift".into()),
            _ => None,
        };
    }
    queue
        .is_none()
        .then(|| previous.map(str::to_owned))
        .flatten()
}
fn draft_champion(draft: &Value) -> Option<u64> {
    let cell = draft["localPlayerCellId"].as_i64()?;
    draft["myTeam"]
        .as_array()?
        .iter()
        .find(|r| r["cellId"].as_i64() == Some(cell))?["championId"]
        .as_u64()
        .filter(|id| *id > 0)
}
fn same_player(row: &Value, player: &Value) -> bool {
    if let (Some(a), Some(b)) = (row["puuid"].as_str(), player["puuid"].as_str()) {
        if !a.is_empty() && !b.is_empty() {
            return a == b;
        }
    }
    ["puuid", "summonerId"].iter().any(|key| {
        let v = &player[*key];
        !v.is_null() && v != 0 && v != "" && row[*key] == *v
    })
}
fn session_champion(session: &Value, player: &Value) -> Option<u64> {
    ["teamOne", "teamTwo", "playerChampionSelections"]
        .iter()
        .filter_map(|k| session["gameData"][*k].as_array())
        .flatten()
        .find(|r| same_player(r, player) && r["championId"].as_u64().is_some_and(|id| id > 0))
        .and_then(|r| r["championId"].as_u64())
}
#[derive(Default)]
struct Tracker {
    phase: String,
    champion: Option<u64>,
    mode: Option<String>,
    game: Option<u64>,
}
impl Tracker {
    fn read(
        &mut self,
        phase: &str,
        session: Option<&Value>,
        draft: Option<&Value>,
        player: Option<&Value>,
    ) {
        let active = matches!(
            phase,
            "ChampSelect" | "GameStart" | "InProgress" | "Reconnect"
        );
        if phase == "ChampSelect" && self.phase != "ChampSelect" {
            self.champion = None;
            self.game = None;
            if !matches!(self.phase.as_str(), "Lobby" | "Matchmaking" | "ReadyCheck") {
                self.mode = None;
            }
        }
        if let Some(s) = session {
            let game = s["gameData"]["gameId"].as_u64().filter(|id| *id > 0);
            if self.game.is_some() && game.is_some() && self.game != game {
                self.champion = None;
                self.mode = None;
            }
            self.game = game.or(self.game);
        }
        // Draft queue information is available before gameflow on some clients;
        // explicit gameflow queues take precedence, including unsupported queues.
        if let Some(draft) = draft {
            self.mode = mode(draft, self.mode.as_deref());
        }
        if let Some(session) = session {
            self.mode = mode(session, self.mode.as_deref());
        }
        if active {
            self.champion = draft.and_then(draft_champion).or(self.champion);
            if phase != "ChampSelect" {
                self.champion = session
                    .and_then(|s| session_champion(s, player?))
                    .or(self.champion);
            }
        } else {
            self.champion = None;
            self.game = None;
            if !matches!(phase, "Lobby" | "Matchmaking" | "ReadyCheck") {
                self.mode = None;
            }
        }
        self.phase = phase.to_owned();
    }
    fn target(&self) -> Value {
        match (self.champion, self.mode.as_deref()) {
            (Some(id), Some(mode)) => json!({"id":id,"mode":mode}),
            _ => Value::Null,
        }
    }
}
fn live(
    http: &reqwest::blocking::Client,
    champions: &Value,
) -> Option<(u64, Option<&'static str>)> {
    let response = http
        .get("https://127.0.0.1:2999/liveclientdata/allgamedata")
        .send()
        .ok()?;
    let data = crate::guide::capped_json(response, 4 * 1024 * 1024).ok()?;
    let active = &data["activePlayer"];
    let player = data["allPlayers"].as_array()?.iter().find(|r| {
        ["riotId", "summonerName"].iter().any(|k| {
            active[*k]
                .as_str()
                .is_some_and(|s| !s.is_empty() && r[*k] == s)
        })
    })?;
    let name = player["championName"].as_str()?;
    let raw = player["rawChampionName"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches("game_character_displayname_");
    let compact = |s: &str| {
        s.to_ascii_lowercase()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
    };
    let id = champions.as_array()?.iter().find(|c| {
        compact(c["name"].as_str().unwrap_or("")) == compact(name)
            || compact(c["name"].as_str().unwrap_or("")) == compact(raw)
    })?["id"]
        .as_u64()?;
    let mode = match data["gameData"]["gameMode"].as_str()? {
        "CLASSIC" | "PRACTICETOOL" => Some("rift"),
        "CHERRY" => Some("arena"),
        _ => None,
    };
    // ARAM live data alone cannot reliably distinguish Mayhem. Ask for a mode in the browser.
    Some((id, mode))
}
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let state = app.state::<Desktop>();
        let Ok(http) = Connection::http(Duration::from_millis(700)) else {
            return;
        };
        let mut connection = None;
        let mut next_poll = Instant::now();
        let mut discovery = Instant::now();
        let mut tracker = Tracker::default();
        let mut player = None;
        let mut shortcut = crate::match_guide::GuideShortcut::default();
        let mut registered = None;
        let mut failed_at = None;
        // Keep draft transitions responsive without requiring an always-open interface.
        let (event_connection, receive_connection) = tokio::sync::watch::channel(None);
        let (event_wake, wake) = std::sync::mpsc::sync_channel(1);
        if !state.preview {
            tauri::async_runtime::spawn(crate::league_connection::events(
                receive_connection,
                event_wake,
            ));
        }
        while !state.is_closing() {
            let now = Instant::now();
            if wake.try_recv().is_ok() {
                next_poll = now;
            }
            if !state.preview && now >= next_poll {
                next_poll = now + Duration::from_secs(2);
                if now >= discovery {
                    let next = Connection::find(None);
                    if next != connection {
                        player = None;
                        if next.is_some() {
                            tracker = Tracker::default();
                        }
                    }
                    connection = next;
                    crate::league_connection::publish(&event_connection, connection.clone());
                    discovery = now + Duration::from_secs(4);
                }
                let fresh = connection
                    .as_ref()
                    .and_then(|c| c.get(&http, "/lol-gameflow/v1/gameflow-phase"))
                    .and_then(|p| p.as_str().map(str::to_owned));
                let connected = fresh.is_some();
                if let Some(phase) = fresh {
                    failed_at = None;
                    let c = connection.as_ref().unwrap();
                    let session = c.get(&http, "/lol-gameflow/v1/session");
                    let draft = matches!(phase.as_str(), "ChampSelect" | "GameStart" | "Reconnect")
                        .then(|| c.get(&http, "/lol-champ-select/v1/session"))
                        .flatten();
                    let entering_match = phase == "ChampSelect" && tracker.phase != phase;
                    let identity_mismatch =
                        matches!(phase.as_str(), "GameStart" | "InProgress" | "Reconnect")
                            && session
                                .as_ref()
                                .is_some_and(|s| s["gameData"]["teamOne"].is_array())
                            && player.as_ref().is_some_and(|p| {
                                session
                                    .as_ref()
                                    .and_then(|s| session_champion(s, p))
                                    .is_none()
                            });
                    if player.is_none() || entering_match || identity_mismatch {
                        player = c.get(&http, "/lol-summoner/v1/current-summoner");
                    }
                    if matches!(phase.as_str(), "Lobby" | "Matchmaking" | "ReadyCheck") {
                        if let Some(lobby) = c.get(&http, "/lol-lobby/v2/lobby") {
                            tracker.mode = mode(&lobby, tracker.mode.as_deref());
                        }
                    }
                    tracker.read(&phase, session.as_ref(), draft.as_ref(), player.as_ref());
                } else if now.duration_since(*failed_at.get_or_insert(now)) > Duration::from_secs(8)
                    && crate::switcher::game_window().is_null()
                {
                    tracker = Tracker::default();
                    tracker.phase = "Offline".into();
                }
                let mut target = tracker.target();
                if (!connected || target.is_null()) && !crate::switcher::game_window().is_null() {
                    let champions = state.view.lock().unwrap()["champions"].clone();
                    if let Some((id, mode)) = live(&http, &champions) {
                        tracker.champion = Some(id);
                        tracker.phase = "InProgress".into();
                        if let Some(mode) = mode {
                            tracker.mode = Some(mode.into());
                        }
                        target = tracker.target();
                    }
                }
                let mut v = state.view.lock().unwrap();
                v["leagueConnected"] = json!(connected);
                v["phase"] = json!(tracker.phase);
                v["guideTarget"] = target;
                v["detectedChampion"] = json!(tracker.champion);
            }
            crate::match_guide::shortcut(&app, &mut shortcut, &mut registered);
            std::thread::sleep(Duration::from_millis(150));
        }
    });
}
#[test]
fn temporary_missing_reads_keep_mode_but_explicit_unknown_queue_clears_it() {
    let mut t = Tracker::default();
    let session = json!({"gameData":{"queue":{"id":2400},"gameId":5}});
    let draft = json!({"localPlayerCellId":2,"myTeam":[{"cellId":1,"championId":103},{"cellId":2,"championId":1}]});
    t.read("ChampSelect", Some(&session), Some(&draft), None);
    assert_eq!(t.target(), json!({"id":1,"mode":"mayhem"}));
    t.read("GameStart", None, None, None);
    assert_eq!(t.target()["id"], 1);
    let final_session = json!({"gameData":{"queue":{"id":2400},"gameId":5,"teamOne":[{"puuid":"local","championId":103},{"puuid":"other","championId":1}]}});
    t.read(
        "InProgress",
        Some(&final_session),
        None,
        Some(&json!({"puuid":"local"})),
    );
    assert_eq!(t.target()["id"], 103, "late swap is authoritative");
    t.read("InProgress", Some(&json!({"queueId":99999})), None, None);
    assert!(t.target().is_null());
    t.read("EndOfGame", None, None, None);
    assert!(t.target().is_null());
}
#[test]
fn a_new_game_never_reuses_the_old_champion() {
    let mut t = Tracker {
        champion: Some(1),
        mode: Some("rift".into()),
        game: Some(1),
        phase: "InProgress".into(),
    };
    t.read(
        "InProgress",
        Some(&json!({"gameData":{"gameId":2,"queue":{"id":450}}})),
        None,
        None,
    );
    assert!(t.target().is_null());
    assert_eq!(t.mode.as_deref(), Some("aram"));
}

#[test]
fn next_draft_cannot_reuse_the_previous_games_mode() {
    let mut t = Tracker {
        phase: "InProgress".into(),
        champion: Some(1),
        mode: Some("mayhem".into()),
        game: Some(5),
    };
    let draft = json!({"localPlayerCellId":2,"myTeam":[{"cellId":2,"championId":103}]});
    t.read("EndOfGame", None, None, None);
    t.read("ChampSelect", None, Some(&draft), None);
    assert!(t.target().is_null());
    let draft =
        json!({"queueId":450,"localPlayerCellId":2,"myTeam":[{"cellId":2,"championId":103}]});
    t.read(
        "ChampSelect",
        Some(&json!({"gameData":{}})),
        Some(&draft),
        None,
    );
    assert_eq!(t.target(), json!({"id":103,"mode":"aram"}));
    t.read(
        "ChampSelect",
        Some(&json!({"queueId":99999})),
        Some(&draft),
        None,
    );
    assert!(
        t.target().is_null(),
        "an explicit unsupported queue must clear the draft fallback"
    );
}

#[test]
fn lobby_mode_survives_into_draft_but_not_a_direct_new_match() {
    let draft = json!({"localPlayerCellId":2,"myTeam":[{"cellId":2,"championId":103}]});
    let mut t = Tracker {
        phase: "Lobby".into(),
        mode: Some("arena".into()),
        ..Default::default()
    };
    t.read("ChampSelect", None, Some(&draft), None);
    assert_eq!(t.target()["mode"], "arena");
    t.phase = "InProgress".into();
    t.read("ChampSelect", None, Some(&draft), None);
    assert!(t.target().is_null());
}

#[test]
fn conflicting_puuids_cannot_match_through_a_shared_legacy_id() {
    assert!(!same_player(
        &json!({"puuid":"other","summonerId":1}),
        &json!({"puuid":"local","summonerId":1})
    ));
    assert!(same_player(
        &json!({"summonerId":1}),
        &json!({"puuid":"local","summonerId":1})
    ));
}
