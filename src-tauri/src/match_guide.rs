use crate::shell::{guide_window, Desktop};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{
    webview::NewWindowResponse, AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Guide windows paint this before their page does, so F1 never flashes white.
pub(crate) const GROUND: tauri::window::Color = tauri::window::Color(16, 16, 18, 255);

pub fn mode(queue: u64) -> Option<&'static str> {
    match queue {
        1700 | 1701 | 1704 | 1710 | 1720 | 1731 | 1732 | 1740 | 1750 => Some("arena"),
        2400 | 2401 | 2403 | 2405 | 2410 | 2450 | 3240 | 3270 | 3280 => Some("mayhem"),
        450 | 62 | 63 | 64 | 65 | 900 | 930 | 931 => Some("aram"),
        400 | 420 | 430 | 440 | 480 | 490 | 700 | 830 | 840 | 850 | 3140 => Some("rift"),
        _ => None,
    }
}
fn slug(name: &str) -> String {
    name.to_ascii_lowercase()
        .split([' ', '-'])
        .map(|s| {
            s.chars()
                .filter(char::is_ascii_alphanumeric)
                .collect::<String>()
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
/// Shows a guide maximized on the monitor the player came from. A window that is already
/// maximized there keeps its size and only comes forward, so showing it costs no relayout.
pub(crate) fn show_maximized(window: &WebviewWindow) -> Result<(), String> {
    let error = |e: tauri::Error| e.to_string();
    if window.is_fullscreen().unwrap_or(false) {
        window.set_fullscreen(false).map_err(error)?;
    }
    let mut moved = false;
    if let Some((x, y)) = crate::switcher::origin_center() {
        if let Some(target) = window.monitor_from_point(x, y).map_err(error)? {
            if window
                .current_monitor()
                .ok()
                .flatten()
                .as_ref()
                .map(|m| *m.position())
                != Some(*target.position())
            {
                window.unmaximize().map_err(error)?;
                window
                    .set_position(target.work_area().position)
                    .map_err(error)?;
                moved = true;
            }
        }
    }
    if moved || !window.is_maximized().unwrap_or(false) {
        window.maximize().map_err(error)?;
    }
    bring_forward(window, false)
}

fn restores_maximized(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, WINDOWPLACEMENT, WPF_RESTORETOMAXIMIZED,
    };
    let mut placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    unsafe {
        GetWindowPlacement(hwnd, &mut placement) != 0
            && placement.flags & WPF_RESTORETOMAXIMIZED != 0
    }
}

/// Brings a guide forward exactly as the player left it: no reload, resize or navigation.
fn bring_forward(window: &WebviewWindow, reused: bool) -> Result<(), String> {
    let error = |e: tauri::Error| e.to_string();
    let hwnd = window.hwnd().map_err(error)?;
    if window.is_minimized().unwrap_or(false) {
        // A plain restore of a maximized guide finished after a quick close and showed the
        // closed guide again; maximizing restores it in one step, as before 5.3.
        if restores_maximized(hwnd.0) {
            window.maximize().map_err(error)?;
        }
        window.unminimize().map_err(error)?;
    }
    window.show().map_err(error)?;
    // show() is applied on the window's own thread; Windows will not focus a window still hidden.
    let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while !window.is_visible().unwrap_or(false) && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    window.set_focus().map_err(error)?;
    let mut focused = crate::switcher::focus(hwnd.0);
    for _ in 0..4 {
        if focused {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
        focused = crate::switcher::focus(hwnd.0);
    }
    if !focused {
        return Err("Windows did not bring the champion guide to the foreground".into());
    }
    let state = window.app_handle().state::<Desktop>();
    crate::support::event(
        &state.root,
        "guide-shown",
        json!({"window":window.label(),"reused":reused}),
    );
    // Only players who have opened a guide get it prepared during later loading screens.
    let used = state.root.join(USED_MARKER);
    if !used.exists() {
        let _ = std::fs::write(used, b"");
    }
    Ok(())
}

fn metasrc_script(options: &serde_json::Value) -> Result<String, String> {
    // The file's last line is a stable marker. Above it sits the factory,
    // `const metasrcGuide = function metasrcGuide() { … };`. The page needs the object that factory
    // returns, so the function expression is invoked before `install` is called with these options.
    // Calling `install` on the bare function threw a TypeError and left MetaSRC's own page showing.
    const END: &str = "globalThis.metasrcGuide = metasrcGuide();";
    const START: &str = "const metasrcGuide = function metasrcGuide()";
    let source = include_str!("../../src/ui/guide/metasrc-guide.js")
        .trim()
        .strip_suffix(END)
        .ok_or("the MetaSRC factory no longer ends with its marker")?;
    let start = source
        .find(START)
        .ok_or("the MetaSRC factory function is missing")?;
    let close = source
        .rfind("};")
        .filter(|close| *close > start)
        .ok_or("the MetaSRC factory function is not closed")?;
    let factory = &source[start + "const metasrcGuide = ".len()..close + 1];
    Ok(format!(
        "if(location.hostname==='www.metasrc.com'){{({factory})().install({options});}}"
    ))
}
const USED_MARKER: &str = "guide-used";

/// What each guide window last loaded for the match, as (label, champion, mode). F1 only brings
/// a guide forward when it already holds the current target, keeping the player's own choices.
static LOADED: Mutex<Vec<(&'static str, u64, String)>> = Mutex::new(Vec::new());

fn label_for(mode: &str) -> &'static str {
    if mode == "mayhem" {
        "guide"
    } else {
        "meta-guide"
    }
}
fn holds(label: &str, id: u64, mode: &str) -> bool {
    LOADED.lock().is_ok_and(|loaded| {
        loaded
            .iter()
            .any(|(l, i, m)| *l == label && *i == id && m == mode)
    })
}
fn remember_loaded(label: &'static str, id: u64, mode: &str) {
    let mut loaded = LOADED.lock().unwrap_or_else(|p| p.into_inner());
    loaded.retain(|(l, ..)| *l != label);
    loaded.push((label, id, mode.to_owned()));
}
fn forget_loaded(label: &str) {
    LOADED
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|(l, ..)| *l != label);
}

fn match_target(view: &serde_json::Value) -> Option<(u64, String)> {
    let target = &view["guideTarget"];
    Some((target["id"].as_u64()?, target["mode"].as_str()?.to_owned()))
}

fn metasrc_url(mode: &str, id: u64, champions: &serde_json::Value) -> Result<tauri::Url, String> {
    let segment = match mode {
        "rift" => "",
        "aram" => "aram/",
        "arena" => "arena/",
        _ => return Err("Unknown guide mode".into()),
    };
    let rows = champions.as_array().ok_or("Champion list unavailable")?;
    let name = rows
        .iter()
        .find(|v| v["key"].as_str().and_then(|s| s.parse::<u64>().ok()) == Some(id))
        .and_then(|v| v["name"].as_str())
        .ok_or("Unknown champion")?;
    tauri::Url::parse(&format!(
        "https://www.metasrc.com/lol/{segment}champions/{}/build",
        slug(name)
    ))
    .map_err(|e| e.to_string())
}

/// Hosts MetaSRC's build pages and its human check need. Everything else on those pages is
/// advertising and analytics (about 110 hosts and 300 requests per page), which the guide hides
/// anyway, so its profile never fetches them.
fn guide_request_allowed(uri: &str) -> bool {
    let Ok(url) = tauri::Url::parse(uri) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") {
        return true;
    }
    url.host_str().is_some_and(|host| {
        host == "metasrc.com"
            || host.ends_with(".metasrc.com")
            || host == "challenges.cloudflare.com"
    })
}

fn block_trackers(window: &WebviewWindow) -> Result<(), String> {
    use webview2_com::{
        take_pwstr, Microsoft::Web::WebView2::Win32::*, WebResourceRequestedEventHandler,
    };
    use windows::core::{Interface, HSTRING, PWSTR};
    window
        .with_webview(|wv| unsafe {
            let Ok(core) = wv.controller().CoreWebView2() else {
                return;
            };
            let Ok(environment) = core
                .cast::<ICoreWebView2_2>()
                .and_then(|core| core.Environment())
            else {
                return;
            };
            if core
                .AddWebResourceRequestedFilter(
                    &HSTRING::from("*"),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                )
                .is_err()
            {
                return;
            }
            let _ = core.add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let mut uri = PWSTR::null();
                    args.Request()?.Uri(&mut uri)?;
                    if !guide_request_allowed(&take_pwstr(uri)) {
                        let blocked = environment.CreateWebResourceResponse(
                            None::<&windows::Win32::System::Com::IStream>,
                            403,
                            &HSTRING::from("Blocked by Oracle"),
                            &HSTRING::new(),
                        )?;
                        args.SetResponse(&blocked)?;
                    }
                    Ok(())
                })),
                &mut 0,
            );
        })
        .map_err(|e| e.to_string())
}

fn create_metasrc(
    app: &AppHandle,
    url: tauri::Url,
    champions: &serde_json::Value,
) -> Result<WebviewWindow, String> {
    let desktop = app.state::<Desktop>();
    let rows = champions.as_array().ok_or("Champion list unavailable")?;
    let appearance = {
        let view = desktop.view.lock().map_err(|_| "State unavailable")?;
        let (colour, theme) = crate::appearance::guide_appearance(&view["settings"]);
        json!({"colour":colour,"theme":theme})
    };
    let catalogue = rows
        .iter()
        .map(|v| json!({"name":v["name"],"slug":slug(v["name"].as_str().unwrap_or(""))}))
        .collect::<Vec<_>>();
    let options = json!({"template":include_str!("../../src/ui/guide/mayhem.html"),"champions":catalogue,"appearance":appearance,
        "style":concat!(include_str!("../../src/ui/appearance.css"),"\n",include_str!("../../src/ui/guide/mayhem.css"),"\n",include_str!("../../src/ui/guide/metasrc-guide.css"),"\n",include_str!("../../src/ui/guide/guide-theme.css"),"\n",include_str!("../../src/ui/designs/eclipse.css"))});
    let script = metasrc_script(&options)?;
    let w = WebviewWindowBuilder::new(app, "meta-guide", WebviewUrl::External(url))
        .title("Oracle · MetaSRC")
        .skip_taskbar(false)
        .visible(false)
        .focused(false)
        .fullscreen(false)
        .background_color(GROUND)
        .data_directory(desktop.root.join("Guide-WebView2"))
        .initialization_script(script)
        .on_page_load(|webview, page| {
            if page.event() == tauri::webview::PageLoadEvent::Finished {
                let desktop = webview.app_handle().state::<Desktop>();
                if let Ok(view) = desktop.view.lock() {
                    let _ = webview.eval(crate::appearance::guide_script(&view["settings"]));
                };
            }
        })
        .on_navigation(|u| {
            u.scheme() == "https"
                && u.host_str() == Some("www.metasrc.com")
                && u.port().is_none()
                && u.path().starts_with("/lol/")
                && u.path().trim_end_matches('/').ends_with("/build")
        })
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false)
        .disable_drag_drop_handler()
        .build()
        .map_err(|e| e.to_string())?;
    if let Err(error) = block_trackers(&w) {
        crate::errorlog::note(&desktop.root, "guide requests", &error);
    }
    if let Some(icon) = crate::appearance::current_icon(app) {
        w.set_icon(icon).map_err(|e| e.to_string())?;
    }
    let closing = w.clone();
    w.on_window_event(move |e| match e {
        // Closing only hides the guide, so the next F1 shows the same page instantly.
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            if closing.is_visible().unwrap_or(false) {
                crate::switcher::return_to_origin();
            }
            let _ = closing.hide();
        }
        tauri::WindowEvent::Destroyed => forget_loaded("meta-guide"),
        _ => {}
    });
    Ok(w)
}

/// Loads `id`/`mode` into its guide window, creating the window hidden when needed.
fn load(
    app: &AppHandle,
    id: u64,
    mode: &str,
    champions: &serde_json::Value,
) -> Result<WebviewWindow, String> {
    let label = label_for(mode);
    let window = if mode == "mayhem" {
        crate::shell::guide_page(app, id)?
    } else {
        let url = metasrc_url(mode, id, champions)?;
        match app.get_webview_window(label) {
            Some(w) => {
                if w.url().ok().as_ref() != Some(&url) {
                    w.navigate(url).map_err(|e| e.to_string())?;
                }
                w
            }
            None => create_metasrc(app, url, champions)?,
        }
    };
    remember_loaded(label, id, mode);
    Ok(window)
}

pub fn open(app: &AppHandle) -> Result<(), String> {
    let desktop = app.state::<Desktop>();
    let target = {
        let v = desktop.view.lock().map_err(|_| "State unavailable")?;
        match_target(&v)
    };
    let Some((id, mode)) = target else {
        return if desktop.preview {
            guide_window(app, 1)
        } else {
            Err("No active match guide".into())
        };
    };
    open_target(app, id, &mode)
}

pub fn manual_open(app: &AppHandle, id: u64, mode: &str) -> Result<(), String> {
    let _guard = OPENING.lock().unwrap_or_else(|p| p.into_inner());
    open_target(app, id, mode)
}
fn open_target(app: &AppHandle, id: u64, mode: &str) -> Result<(), String> {
    let champions = app
        .state::<Desktop>()
        .view
        .lock()
        .map_err(|_| "State unavailable")?["champions"]
        .clone();
    let label = label_for(&mode);
    let existing = app.get_webview_window(label);
    let visible = existing
        .as_ref()
        .is_some_and(|w| w.is_visible().unwrap_or(false));
    crate::switcher::remember_origin(visible);
    match existing.filter(|_| holds(label, id, &mode)) {
        // F1 on an open guide only focuses it; a closed one reappears without reloading.
        Some(w) if visible => bring_forward(&w, true),
        Some(w) => show_maximized(&w),
        None => show_maximized(&load(app, id, &mode, &champions)?),
    }
}

static PREWARMING: AtomicBool = AtomicBool::new(false);
static PREWARMED: Mutex<Option<(u64, String)>> = Mutex::new(None);

/// Prepares the current match's guide hidden during the loading screen, so the first F1 shows it
/// at once. Only for players who have used a guide before and have a guide key.
fn prewarm(app: &AppHandle, view: &serde_json::Value) {
    let target = match_target(view);
    {
        let mut last = PREWARMED.lock().unwrap_or_else(|p| p.into_inner());
        if *last == target {
            return;
        }
        last.clone_from(&target);
    }
    let Some((id, mode)) = target else { return };
    let state = app.state::<Desktop>();
    if !view["settings"]["guideHotkey"].is_string()
        || view["guideCapture"] == true
        || state.is_closing()
        || !state.root.join(USED_MARKER).exists()
    {
        return;
    }
    let champions = view["champions"].clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        PREWARMING.store(true, Ordering::SeqCst);
        let result = match OPENING.try_lock() {
            Ok(_opening) => {
                let label = label_for(&mode);
                let current = app.get_webview_window(label);
                if current
                    .as_ref()
                    .is_some_and(|w| w.is_visible().unwrap_or(false))
                    || (current.is_some() && holds(label, id, &mode))
                {
                    Ok(())
                } else {
                    load(&app, id, &mode, &champions).map(drop)
                }
            }
            Err(_) => Ok(()),
        };
        PREWARMING.store(false, Ordering::SeqCst);
        if let Err(error) = result {
            crate::errorlog::note(&app.state::<Desktop>().root, "guide prewarm", &error);
        }
    });
}
#[derive(Default, PartialEq, Eq)]
pub enum GuideShortcut {
    #[default]
    NoMatch,
    InMatch(Option<String>),
}

impl GuideShortcut {
    fn from_view(view: &serde_json::Value) -> Self {
        if view["guideTarget"].is_null() {
            return Self::NoMatch;
        }
        let binding = view["settings"]["guideHotkey"]
            .as_str()
            .filter(|_| view["guideCapture"] != true && view["hotkeyContext"] != false)
            .map(str::to_owned);
        Self::InMatch(binding)
    }
}

pub fn shortcut(app: &AppHandle, current: &mut GuideShortcut, registered: &mut Option<String>) {
    let state = app.state::<Desktop>();
    let Ok(view) = state.view.lock() else { return };
    let wanted = GuideShortcut::from_view(&view);
    prewarm(app, &view);
    drop(view);
    if *current == wanted {
        return;
    }
    *current = wanted;
    if let Some(old) = registered.take() {
        let _ = app.global_shortcut().unregister(old.as_str());
    }
    if let GuideShortcut::InMatch(Some(binding)) = current {
        let result = app
            .global_shortcut()
            .on_shortcut(binding.as_str(), |app, _, event| {
                if event.state == ShortcutState::Pressed {
                    // WebView2 creation can wait for the same Windows event loop
                    // that delivered WM_HOTKEY. Let the callback return immediately.
                    open_from_shortcut(app);
                }
            });
        if result.is_ok() {
            *registered = Some(binding.clone());
        }
        if let Ok(mut view) = state.view.lock() {
            view["shortcutStatus"] = json!(if result.is_ok() { "ready" } else { "conflict" });
        }
        crate::support::event(
            &state.root,
            "guide-shortcut",
            json!({
                "binding":binding,"registered":result.is_ok(),"pollingFallback":true,
                "error":result.err().map(|error|error.to_string())
            }),
        );
    } else if matches!(current, GuideShortcut::NoMatch) {
        if let Ok(mut view) = state.view.lock() {
            view["shortcutStatus"] = json!("waiting");
        }
        if let Some(w) = app.get_webview_window("guide") {
            let _ = w.hide();
        }
        // MetaSRC runs in its own browser process; free it between matches.
        if let Some(w) = app.get_webview_window("meta-guide") {
            let _ = w.destroy();
        }
        forget_loaded("meta-guide");
    }
}
static OPENING: Mutex<()> = Mutex::new(());

/// The guide lock for F1. A press while the guide is being prepared in the background waits for
/// it instead of being dropped; a press while another F1 is opening is a repeat and is dropped.
fn opening_for_key() -> Option<std::sync::MutexGuard<'static, ()>> {
    match OPENING.try_lock() {
        Ok(guard) => Some(guard),
        Err(std::sync::TryLockError::WouldBlock) if PREWARMING.load(Ordering::SeqCst) => {
            Some(OPENING.lock().unwrap_or_else(|p| p.into_inner()))
        }
        Err(std::sync::TryLockError::WouldBlock) => None,
        Err(std::sync::TryLockError::Poisoned(p)) => Some(p.into_inner()),
    }
}

fn shortcut_enabled(app: &AppHandle) -> bool {
    let state = app.state::<Desktop>();
    !state.is_closing()
        && state.view.lock().is_ok_and(|v| {
            !v["guideTarget"].is_null()
                && v["guideCapture"] != true
                && crate::switcher::hotkey_context(app)
        })
}

pub(crate) fn open_from_activation(app: &AppHandle, hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let Some(_guard) = opening_for_key() else {
        return;
    };
    if unsafe { GetForegroundWindow() } == hwnd {
        if !shortcut_enabled(app) {
            crate::switcher::return_to_origin();
        } else if let Err(error) = open(app) {
            crate::errorlog::note(&app.state::<Desktop>().root, "guide activation", &error);
            if unsafe { GetForegroundWindow() } == hwnd {
                crate::switcher::return_to_origin();
            }
        }
    }
}

pub(crate) fn open_from_shortcut(app: &AppHandle) {
    use std::time::{Duration, Instant};
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
    if last.is_some_and(|at| at.elapsed() < Duration::from_millis(350)) {
        return;
    }
    *last = Some(Instant::now());
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(_opening) = opening_for_key() else {
            return;
        };
        if !shortcut_enabled(&app) {
            return;
        }
        if let Err(error) = open(&app) {
            crate::errorlog::note(&app.state::<Desktop>().root, "guide hotkey", &error);
        }
    });
}
#[test]
fn the_metasrc_initialization_script_wraps_the_named_factory_once() {
    let script = metasrc_script(&serde_json::json!({"template":"<html></html>"})).unwrap();
    assert!(
        script.starts_with("if(location.hostname==='www.metasrc.com'){(function metasrcGuide()")
    );
    assert!(
        script.ends_with(").install({\"template\":\"<html></html>\"});}"),
        "{script}"
    );
    assert!(script.matches(".install(").count() == 1);
    assert!(
        script.contains("})().install("),
        "the factory must be invoked before install"
    );
    assert!(
        !script.contains("}).install("),
        "install is not a property of the bare factory"
    );
}

#[test]
fn match_end_is_distinct_from_capture_and_does_not_depend_on_registration() {
    let mut view = serde_json::json!({"guideTarget":{"id":1}, "settings":{"guideHotkey":"F1"}});
    assert!(matches!(
        GuideShortcut::from_view(&view),
        GuideShortcut::InMatch(Some(_))
    ));
    view["guideCapture"] = true.into();
    assert!(matches!(
        GuideShortcut::from_view(&view),
        GuideShortcut::InMatch(None)
    ));
    view["guideTarget"] = serde_json::Value::Null;
    assert!(matches!(
        GuideShortcut::from_view(&view),
        GuideShortcut::NoMatch
    ));
}

#[test]
fn guide_mode_and_champion_spelling() {
    assert_eq!(mode(2400), Some("mayhem"));
    assert_eq!(mode(1700), Some("arena"));
    assert_eq!(mode(99999), None);
    assert_eq!(slug("Kai'Sa"), "kaisa");
    assert_eq!(slug("Dr. Mundo"), "dr-mundo");
}

#[test]
fn guide_windows_remember_what_they_hold() {
    forget_loaded("guide");
    assert!(!holds("guide", 103, "mayhem"));
    remember_loaded(label_for("mayhem"), 103, "mayhem");
    assert!(holds("guide", 103, "mayhem"));
    assert!(!holds("guide", 104, "mayhem"), "another champion must load");
    assert!(!holds("meta-guide", 103, "mayhem"));
    remember_loaded("guide", 104, "mayhem");
    assert!(!holds("guide", 103, "mayhem"), "one target per window");
    forget_loaded("guide");
    assert!(!holds("guide", 104, "mayhem"));
    assert_eq!(label_for("arena"), "meta-guide");
    assert_eq!(
        match_target(&serde_json::json!({"guideTarget":{"id":5,"mode":"aram"}})),
        Some((5, "aram".into()))
    );
    assert_eq!(match_target(&serde_json::json!({"guideTarget":null})), None);
}

#[test]
fn the_metasrc_guide_only_fetches_metasrc_and_its_human_check() {
    for allowed in [
        "https://www.metasrc.com/lol/arena/champions/ahri/build",
        "https://cdn.metasrc.com/combo/js/htmx.js",
        "https://www.metasrc.com/cdn-cgi/challenge-platform/h/b/orchestrate/chl_page/v1",
        "https://challenges.cloudflare.com/turnstile/v0/api.js",
        "data:image/png;base64,AAAA",
        "blob:https://www.metasrc.com/1234",
    ] {
        assert!(guide_request_allowed(allowed), "{allowed}");
    }
    for blocked in [
        "https://www.googletagmanager.com/gtag/js",
        "https://securepubads.g.doubleclick.net/tag/js/gpt.js",
        "https://metasrc.com.evil.example/x",
        "https://evilmetasrc.com/x",
        "not a url",
    ] {
        assert!(!guide_request_allowed(blocked), "{blocked}");
    }
}
