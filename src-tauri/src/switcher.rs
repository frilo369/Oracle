//! Windows can land on Riot Client when a fullscreen game minimizes. During a
//! supported match, correct only a single Alt+Tab between the game and the guide,
//! and return to the window the guide was opened from when the player closes it.
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND},
    System::Threading::{
        AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW,
        PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_MENU, VK_TAB},
        WindowsAndMessaging::{
            FindWindowExW, GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
            IsWindow, IsWindowVisible, SendMessageTimeoutW, SetForegroundWindow, ShowWindowAsync,
            SMTO_ABORTIFHUNG, SMTO_BLOCK, SW_RESTORE, WM_NULL,
        },
    },
};

/// A window outside this app, with the process that owned it when it was seen.
type Seen = (usize, u32);
/// The last window in front that did not belong to this app.
static PREVIOUS: Mutex<Option<Seen>> = Mutex::new(None);
/// Where the open guide was opened from.
static ORIGIN: Mutex<Option<Seen>> = Mutex::new(None);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Game,
    Guide,
    League,
    Riot,
    Own,
    Switcher,
    Other,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Window {
    handle: usize,
    kind: Kind,
}
struct Chord {
    origin: Window,
    taps: u8,
    expires: Option<Instant>,
}
#[derive(Default)]
struct Switch {
    previous: Option<Window>,
    tab_down: bool,
    chord: Option<Chord>,
}
impl Switch {
    fn step(
        &mut self,
        now: Instant,
        alt: bool,
        tab: bool,
        front: Option<Window>,
        guide: bool,
    ) -> Option<Kind> {
        let press = tab && !self.tab_down;
        self.tab_down = tab;
        if !alt {
            self.previous = front;
        }
        if alt && press {
            if let Some(chord) = self.chord.as_mut().filter(|c| c.expires.is_none()) {
                chord.taps = chord.taps.saturating_add(1);
            } else {
                self.chord = self.previous.map(|origin| Chord {
                    origin,
                    taps: 1,
                    expires: None,
                });
            }
            return None;
        }
        let chord = self.chord.as_mut()?;
        if alt && chord.expires.is_none() {
            return None;
        }
        let deadline = *chord
            .expires
            .get_or_insert(now + Duration::from_millis(700));
        if now > deadline || chord.taps != 1 {
            self.chord = None;
            return None;
        }
        let landed = front?;
        if landed.handle == chord.origin.handle || landed.kind == Kind::Switcher {
            return None;
        }
        let target = match chord.origin.kind {
            Kind::Game if guide => Some(Kind::Guide),
            Kind::Guide if guide => Some(Kind::Game),
            Kind::League | Kind::Own if matches!(landed.kind, Kind::League | Kind::Riot) => {
                Some(Kind::Game)
            }
            _ => None,
        };
        self.chord = None;
        target.filter(|t| *t != landed.kind)
    }
}
pub(crate) fn image_path(pid: u32) -> Option<PathBuf> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut n = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut n);
        CloseHandle(handle);
        (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..n as usize])))
    }
}
fn process(hwnd: HWND) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid
}
/// The visible game window. As in 13.7, a process Windows will not name still counts.
pub(crate) fn game_window() -> HWND {
    let title: Vec<_> = "League of Legends (TM) Client"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut hwnd = std::ptr::null_mut();
    loop {
        hwnd =
            unsafe { FindWindowExW(std::ptr::null_mut(), hwnd, std::ptr::null(), title.as_ptr()) };
        if hwnd.is_null() {
            return hwnd;
        }
        if unsafe { IsWindowVisible(hwnd) } != 0
            && image_path(process(hwnd)).is_none_or(|p| {
                p.file_name()
                    .is_some_and(|s| s.eq_ignore_ascii_case("League of Legends.exe"))
            })
        {
            return hwnd;
        }
    }
}
/// Brings a window forward. Windows refuses a process that is not in front, so as in
/// 13.7 the input of the window in front is attached for one more attempt, but only
/// when that window still answers messages.
pub(crate) fn focus(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }
    unsafe {
        if GetForegroundWindow() == hwnd {
            return true;
        }
        SetForegroundWindow(hwnd);
        if GetForegroundWindow() == hwnd {
            return true;
        }
        let current = GetForegroundWindow();
        if current.is_null() {
            return false;
        }
        let mut ignored = 0;
        if SendMessageTimeoutW(
            current,
            WM_NULL,
            0,
            0,
            SMTO_BLOCK | SMTO_ABORTIFHUNG,
            50,
            &mut ignored,
        ) == 0
        {
            return false;
        }
        let other = GetWindowThreadProcessId(current, std::ptr::null_mut());
        let own = GetCurrentThreadId();
        if other != 0 && other != own {
            if AttachThreadInput(own, other, 1) == 0 {
                return false;
            }
            SetForegroundWindow(hwnd);
            AttachThreadInput(own, other, 0);
        } else {
            SetForegroundWindow(hwnd);
        }
        GetForegroundWindow() == hwnd
    }
}
/// Brings the running game forward. Restoring a minimized game activates it, and in
/// 13.7 a second activation on top made a game that minimizes when it loses focus
/// drop straight back down, so only step in if it has not come up after 80 ms.
fn focus_game() -> bool {
    let game = game_window();
    if game.is_null() {
        return false;
    }
    if unsafe { IsIconic(game) } == 0 {
        return focus(game);
    }
    unsafe { ShowWindowAsync(game, SW_RESTORE) };
    let handle = game as usize;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(80));
        let game = handle as HWND;
        if unsafe { GetForegroundWindow() } != game {
            focus(game);
        }
    });
    true
}
/// Records where a guide is opening from: the window in front, or the last outside
/// window when this app is already in front and the guide was hidden.
pub(crate) fn remember_origin(guide_visible: bool) {
    let front = unsafe { GetForegroundWindow() };
    let pid = if front.is_null() { 0 } else { process(front) };
    let seen = if pid != 0 && pid != std::process::id() {
        Some((front as usize, pid))
    } else if !guide_visible {
        PREVIOUS.lock().ok().and_then(|p| *p)
    } else {
        return;
    };
    if let (Some(seen), Ok(mut origin)) = (seen, ORIGIN.lock()) {
        *origin = Some(seen);
    }
}
/// The player closed the guide: go back to the window it was opened from, as long as
/// that window still belongs to the same process.
pub(crate) fn origin_center() -> Option<(f64, f64)> {
    let (handle, pid) = (*ORIGIN.lock().ok()?)?;
    let hwnd = handle as HWND;
    let mut rect = windows_sys::Win32::Foundation::RECT::default();
    if process(hwnd) != pid
        || unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect) }
            == 0
        || unsafe { IsIconic(hwnd) } != 0
    {
        return None;
    }
    Some((
        (f64::from(rect.left) + f64::from(rect.right)) / 2.,
        (f64::from(rect.top) + f64::from(rect.bottom)) / 2.,
    ))
}
pub(crate) fn return_to_origin() -> bool {
    let Some((handle, pid)) = ORIGIN.lock().ok().and_then(|mut o| o.take()) else {
        return false;
    };
    let hwnd = handle as HWND;
    unsafe {
        if pid == std::process::id() || IsWindow(hwnd) == 0 || process(hwnd) != pid {
            return false;
        }
        if IsIconic(hwnd) != 0 {
            ShowWindowAsync(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd) != 0
    }
}
fn classify(hwnd: HWND, game: HWND, guide: HWND) -> Option<Window> {
    if hwnd.is_null() {
        return None;
    }
    let kind = if hwnd == game {
        Kind::Game
    } else if hwnd == guide {
        Kind::Guide
    } else {
        let pid = process(hwnd);
        if pid == std::process::id() {
            Kind::Own
        } else {
            let mut name = [0u16; 128];
            let len = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), name.len() as i32) };
            let class = String::from_utf16_lossy(&name[..len.max(0) as usize]);
            if matches!(
                class.as_str(),
                "XamlExplorerHostIslandWindow"
                    | "MultitaskingViewFrame"
                    | "TaskSwitcherWnd"
                    | "ForegroundStaging"
            ) {
                Kind::Switcher
            } else {
                match image_path(pid)
                    .and_then(|p| {
                        p.file_name()
                            .map(|s| s.to_string_lossy().to_ascii_lowercase())
                    })
                    .as_deref()
                {
                    Some("leagueclientux.exe") => Kind::League,
                    Some(
                        "riotclientux.exe" | "riotclient.exe" | "riot client.exe"
                        | "riot clientux.exe",
                    ) => Kind::Riot,
                    _ => Kind::Other,
                }
            }
        }
    };
    Some(Window {
        handle: hwnd as usize,
        kind,
    })
}
pub(crate) fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut switch = Switch::default();
        let mut cached = None;
        let mut game = std::ptr::null_mut();
        let mut guide = std::ptr::null_mut();
        let mut refresh = Instant::now();
        let mut binding = None;
        let mut key_down = false;
        let mut activation: Option<crate::guide_keys::Activation> = None;
        let state = app.state::<crate::shell::Desktop>();
        while !state.is_closing() {
            let now = Instant::now();
            if now >= refresh {
                refresh = now + Duration::from_millis(300);
                // As in 13.7, only while a guide hotkey is ready and not being recorded.
                let target = state
                    .view
                    .lock()
                    .ok()
                    .filter(|v| {
                        v["guideCapture"] != true && v["settings"]["guideHotkey"].is_string()
                    })
                    .and_then(|v| {
                        Some((
                            v["guideTarget"]["mode"].as_str()?.to_owned(),
                            crate::guide_keys::Binding::parse(
                                v["settings"]["guideHotkey"].as_str()?,
                            )?,
                        ))
                    });
                let Some((mode, next)) = target else {
                    activation = None;
                    binding = None;
                    refresh = now;
                    switch = Switch::default();
                    cached = None;
                    std::thread::sleep(Duration::from_millis(300));
                    continue;
                };
                if binding != Some(next) {
                    activation = next
                        .activation_word()
                        .and_then(|_| crate::guide_keys::Activation::new());
                    key_down = next.key_down();
                    binding = Some(next);
                }
                game = game_window();
                guide = app
                    .get_webview_window(if mode == "mayhem" {
                        "guide"
                    } else {
                        "meta-guide"
                    })
                    .filter(|w| w.is_visible().unwrap_or(false))
                    .and_then(|w| w.hwnd().ok())
                    .map_or(std::ptr::null_mut(), |h| h.0);
            }
            if let Some(helper) = &activation {
                helper.pump();
            }
            let hwnd = unsafe { GetForegroundWindow() };
            let Some(keys) = binding else { continue };
            let down = keys.key_down();
            let activated = activation
                .as_ref()
                .is_some_and(|helper| helper.hwnd == hwnd);
            if let Some(helper) = &mut activation {
                if activated || hwnd.is_null() || process(hwnd) == std::process::id() {
                    helper.disarm();
                } else if !down && hwnd == game {
                    if let Some(word) = keys.activation_word() {
                        helper.arm(word);
                    }
                }
            }
            if activated {
                crate::match_guide::open_from_activation(&app, hwnd);
            } else if down && !key_down && keys.modifiers_down() && hotkey_context(&app) {
                crate::match_guide::open_from_shortcut(&app);
            }
            key_down = down;
            let key = (hwnd as usize, game as usize, guide as usize);
            if cached.as_ref().is_none_or(|(k, _)| *k != key) {
                let seen = classify(hwnd, game, guide);
                if let Some(w) = seen.filter(|w| !matches!(w.kind, Kind::Own | Kind::Switcher)) {
                    if let Ok(mut previous) = PREVIOUS.lock() {
                        *previous = Some((w.handle, process(hwnd)));
                    }
                }
                cached = Some((key, seen))
            }
            let alt = unsafe { GetAsyncKeyState(VK_MENU as i32) } < 0;
            let tab = unsafe { GetAsyncKeyState(VK_TAB as i32) } < 0;
            if let Some(kind) = switch.step(
                now,
                alt,
                tab,
                cached.as_ref().and_then(|(_, w)| *w),
                !guide.is_null(),
            ) {
                if kind == Kind::Game {
                    focus_game();
                } else if unsafe { IsWindowVisible(guide) } != 0 {
                    if unsafe { IsIconic(guide) } != 0 {
                        unsafe { ShowWindowAsync(guide, SW_RESTORE) };
                    } else {
                        focus(guide);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(8));
        }
    });
}
pub(crate) fn hotkey_context(app: &AppHandle) -> bool {
    let front = unsafe { GetForegroundWindow() };
    if front.is_null() {
        return false;
    }
    if front == game_window() {
        return true;
    }
    if app.state::<crate::shell::Desktop>().preview {
        let mut text = [0u16; 64];
        let n = unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW(
                front,
                text.as_mut_ptr(),
                64,
            )
        };
        if String::from_utf16_lossy(&text[..n.max(0) as usize]) == "Oracle test game" {
            return true;
        }
    }
    ["guide", "meta-guide"].iter().any(|label| {
        app.get_webview_window(label)
            .and_then(|w| w.hwnd().ok())
            .is_some_and(|h| h.0 == front)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn replay(from: Kind, land: Kind, guide: bool, taps: usize, delay: u64) -> Option<Kind> {
        let mut s = Switch::default();
        let n = Instant::now();
        let source = Some(Window {
            handle: 1,
            kind: from,
        });
        let picker = Some(Window {
            handle: 2,
            kind: Kind::Switcher,
        });
        s.step(n, false, false, source, guide);
        s.step(n, true, false, source, guide);
        for _ in 0..taps {
            s.step(n, true, true, picker, guide);
            s.step(n, true, false, picker, guide);
        }
        assert_eq!(s.step(n, false, false, picker, guide), None);
        s.step(
            n + Duration::from_millis(delay),
            false,
            false,
            Some(Window {
                handle: 3,
                kind: land,
            }),
            guide,
        )
    }
    #[test]
    fn single_switch_follows_open_guide_and_game() {
        for land in [Kind::Riot, Kind::Other] {
            assert_eq!(replay(Kind::Guide, land, true, 1, 15), Some(Kind::Game));
            assert_eq!(replay(Kind::Game, land, true, 1, 15), Some(Kind::Guide));
        }
        assert_eq!(replay(Kind::Guide, Kind::Game, true, 1, 15), None);
        assert_eq!(replay(Kind::Game, Kind::Guide, true, 1, 15), None);
        assert_eq!(replay(Kind::Game, Kind::Other, false, 1, 15), None);
    }
    #[test]
    fn client_switch_only_redirects_a_launcher() {
        assert_eq!(
            replay(Kind::League, Kind::Riot, false, 1, 15),
            Some(Kind::Game)
        );
        assert_eq!(replay(Kind::League, Kind::Other, true, 1, 15), None);
    }
    #[test]
    fn deliberate_second_tab_or_later_click_keeps_windows_choice() {
        assert_eq!(replay(Kind::Game, Kind::Riot, true, 2, 15), None);
        assert_eq!(replay(Kind::Guide, Kind::Other, true, 1, 701), None);
    }
}
