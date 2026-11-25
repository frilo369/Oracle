use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub struct Desktop {
    pub root: PathBuf,
    pub view: Mutex<Value>,
    pub preview: bool,
    pub closing: AtomicBool,
    save_gate: Mutex<()>,
}
impl Desktop {
    pub fn new(root: PathBuf, preview: bool) -> Self {
        let (settings, error) = crate::settings::load(&root);
        let catalogue: Value =
            serde_json::from_str(include_str!("../../src/ui/guide/mayhem-catalogue.json"))
                .expect("bundled catalogue");
        let mut champions=catalogue["champions"].as_object().unwrap().iter().map(|(id,row)|json!({"key":id,"id":id.parse::<u64>().unwrap_or(0),"name":row["name"],"icon":row["icon"]})).collect::<Vec<_>>();
        champions.sort_by_key(|c| c["name"].as_str().unwrap_or("").to_owned());
        Self {
            root,
            preview,
            closing: AtomicBool::new(false),
            save_gate: Mutex::new(()),
            view: Mutex::new(
                json!({"version":env!("CARGO_PKG_VERSION"),"settings":settings,"champions":champions,"guideTarget":if preview {json!({"id":1,"mode":"mayhem"})} else {Value::Null},"guideCapture":false,"hotkeyContext":false,"leagueConnected":false,"phase":if preview{"Fixture"}else{"Offline"},"shortcutStatus":"waiting","error":error}),
            ),
        }
    }
    pub fn is_closing(&self) -> bool {
        self.closing.load(Ordering::Relaxed)
    }
}
pub fn report(app: &AppHandle, error: &str) {
    let state = app.state::<Desktop>();
    if let Ok(mut v) = state.view.lock() {
        v["error"] = json!(error);
    }
    let _ = show_dashboard(app);
    let _ = app.emit("oracle-error", error);
}
pub fn show_dashboard(app: &AppHandle) -> Result<(), String> {
    static CREATING: Mutex<()> = Mutex::new(());
    let _creating = CREATING.lock().map_err(|_| "Window unavailable")?;
    let w = match app.get_webview_window("dashboard") {
        Some(w) => w,
        None => {
            let w =
                WebviewWindowBuilder::new(app, "dashboard", WebviewUrl::App("index.html".into()))
                    .title("Oracle")
                    .inner_size(580., 610.)
                    .min_inner_size(520., 570.)
                    .center()
                    .decorations(false)
                    .background_color(crate::match_guide::GROUND)
                    .data_directory(app.state::<Desktop>().root.join("Dashboard-WebView2"))
                    .disable_drag_drop_handler()
                    .on_navigation(|u| {
                        u.host_str() == Some("tauri.localhost") || u.scheme() == "tauri"
                    })
                    .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                    .on_download(|_, _| false)
                    .build()
                    .map_err(|e| e.to_string())?;
            let copy = w.clone();
            let handle = app.clone();
            w.on_window_event(move |e| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                    api.prevent_close();
                    let _ = copy.hide();
                    if let Ok(mut v) = handle.state::<Desktop>().view.lock() {
                        v["guideCapture"] = json!(false);
                    }
                }
            });
            w
        }
    };
    w.unminimize().map_err(|e| e.to_string())?;
    w.show().map_err(|e| e.to_string())?;
    w.set_focus().map_err(|e| e.to_string())
}
pub fn guide_page(app: &AppHandle, champion: u64) -> Result<WebviewWindow, String> {
    let w = if let Some(w) = app.get_webview_window("guide") {
        w.eval(format!("window.umbraShowChampion?window.umbraShowChampion({champion}):location.replace('/guide/mayhem.html?champion={champion}')")).map_err(|e|e.to_string())?;
        w
    } else {
        let w = WebviewWindowBuilder::new(
            app,
            "guide",
            WebviewUrl::App(format!("guide/mayhem.html?champion={champion}").into()),
        )
        .title("Oracle · ARAM Mayhem")
        .inner_size(1100., 760.)
        .center()
        .skip_taskbar(false)
        .fullscreen(false)
        .visible(false)
        .focused(false)
        .background_color(crate::match_guide::GROUND)
        .data_directory(app.state::<Desktop>().root.join("Dashboard-WebView2"))
        .initialization_script(crate::appearance::guide_script(
            &app.state::<Desktop>().view.lock().unwrap()["settings"],
        ))
        .on_navigation(|u| {
            (u.host_str() == Some("tauri.localhost") || u.scheme() == "tauri")
                && u.path() == "/guide/mayhem.html"
        })
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .on_download(|_, _| false)
        .disable_drag_drop_handler()
        .on_page_load(|wv, page| {
            if page.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = wv.eval(crate::appearance::guide_script(
                    &wv.app_handle().state::<Desktop>().view.lock().unwrap()["settings"],
                ));
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
        let copy = w.clone();
        w.on_window_event(move |e| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                api.prevent_close();
                if copy.is_visible().unwrap_or(false) {
                    crate::switcher::return_to_origin();
                }
                let _ = copy.hide();
            }
        });
        w
    };
    Ok(w)
}
pub fn guide_window(app: &AppHandle, champion: u64) -> Result<(), String> {
    crate::match_guide::show_maximized(&guide_page(app, champion)?)
}

#[tauri::command]
pub fn ready(state: tauri::State<'_, Desktop>) -> Result<Value, String> {
    state
        .view
        .lock()
        .map(|v| v.clone())
        .map_err(|_| "State unavailable".into())
}

#[tauri::command]
pub async fn action(app: AppHandle, kind: String, id: Value) -> Result<Value, String> {
    let state = app.state::<Desktop>();
    match kind.as_str() {
        "capture" => {
            state.view.lock().map_err(|_| "State unavailable")?["guideCapture"] = json!(id == true);
        }
        "save-settings" => {
            let _gate = state.save_gate.lock().map_err(|_| "Settings unavailable")?;
            let mut next = state.view.lock().map_err(|_| "State unavailable")?["settings"].clone();
            for (key, value) in id.as_object().ok_or("Invalid settings")? {
                if matches!(key.as_str(), "notes" | "favorites" | "trayDefault13") {
                    return Err("This preference cannot be changed here".into());
                }
                if !crate::settings::valid(key, value) {
                    return Err(format!("Invalid setting: {key}"));
                }
                next[key] = value.clone();
            }
            let old = state.view.lock().map_err(|_| "State unavailable")?["settings"].clone();
            let startup_changed = old["startWithWindows"] != next["startWithWindows"];
            if startup_changed && !state.preview {
                crate::settings::startup(next["startWithWindows"] == true)?;
            }
            if let Err(e) = crate::settings::save(&state.root.join("settings.json"), &next) {
                if startup_changed && !state.preview {
                    let _ = crate::settings::startup(old["startWithWindows"] == true);
                }
                return Err(e);
            }
            {
                let mut view = state.view.lock().map_err(|_| "State unavailable")?;
                view["settings"] = next.clone();
                view["error"] = Value::Null;
            }
            for label in ["guide", "meta-guide"] {
                if let Some(w) = app.get_webview_window(label) {
                    let _ = w.eval(crate::appearance::guide_script(&next));
                }
            }
        }
        "open-guide" => {
            let champ = id["champion"].as_u64().ok_or("Choose a champion")?;
            let mode = id["mode"]
                .as_str()
                .filter(|m| matches!(*m, "rift" | "aram" | "arena" | "mayhem"))
                .ok_or("Choose a supported mode")?
                .to_owned();
            if !state.view.lock().unwrap()["champions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["id"] == champ)
            {
                return Err("Unknown champion".into());
            }
            let handle = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                crate::match_guide::manual_open(&handle, champ, &mode)
            })
            .await
            .map_err(|e| e.to_string())??;
        }
        "open-current" => {
            let handle = app.clone();
            tauri::async_runtime::spawn_blocking(move || crate::match_guide::open(&handle))
                .await
                .map_err(|e| e.to_string())??;
        }
        "minimize" => {
            app.get_webview_window("dashboard")
                .ok_or("Window unavailable")?
                .minimize()
                .map_err(|e| e.to_string())?;
        }
        "maximize" => {
            let w = app
                .get_webview_window("dashboard")
                .ok_or("Window unavailable")?;
            if w.is_maximized().unwrap_or(false) {
                w.unmaximize()
            } else {
                w.maximize()
            }
            .map_err(|e| e.to_string())?;
        }
        "hide" => {
            state.view.lock().unwrap()["guideCapture"] = json!(false);
            app.get_webview_window("dashboard")
                .ok_or("Window unavailable")?
                .hide()
                .map_err(|e| e.to_string())?;
        }
        "quit" => app.exit(0),
        "dismiss-error" => state.view.lock().unwrap()["error"] = Value::Null,
        "fixture" => {
            if !state.preview {
                return Err("Fixture mode is disabled".into());
            }
            if let Some(target) = id.get("target") {
                state.view.lock().unwrap()["guideTarget"] = target.clone();
            }
            if id["activation"] == true {
                // Exercise the real own-process helper becoming foreground, as WM_SETHOTKEY
                // does from fullscreen. No synthetic key or polling shortcut opens this guide.
                let handle = app.clone();
                tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
                    let helper = crate::guide_keys::Activation::new()
                        .ok_or("Activation helper unavailable")?;
                    helper.pump();
                    if !crate::switcher::focus(helper.hwnd) {
                        return Err("Activation helper did not take foreground".into());
                    }
                    crate::match_guide::open_from_activation(&handle, helper.hwnd);
                    Ok(())
                })
                .await
                .map_err(|e| e.to_string())??;
            }
            let mut status = json!({});
            if let Some(w) = app.get_webview_window("guide") {
                if id["close"] == true {
                    w.close().map_err(|e| e.to_string())?;
                }
                if id["minimize"] == true {
                    w.minimize().map_err(|e| e.to_string())?;
                }
                status = json!({"visible":w.is_visible().unwrap_or(false),"maximized":w.is_maximized().unwrap_or(false),"minimized":w.is_minimized().unwrap_or(false),"fullscreen":w.is_fullscreen().unwrap_or(false),"foreground":w.hwnd().ok().is_some_and(|h|unsafe{windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow()==h.0})});
            }
            let front =
                unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
            let mut front_pid = 0;
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                    front,
                    &mut front_pid,
                );
            }
            status["frontPid"] = json!(front_pid);
            status["appPid"] = json!(std::process::id());
            status["frontProcess"] = json!(crate::switcher::image_path(front_pid)
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())));
            return Ok(status);
        }
        _ => return Err("Unknown action".into()),
    }
    ready(state)
}
#[tauri::command]
pub async fn guide_data(app: AppHandle, command: String, args: Value) -> Result<Value, String> {
    let state = app.state::<Desktop>();
    let root = state.root.clone();
    let fixture = state.preview;
    let response = tauri::async_runtime::spawn_blocking(move || match command.as_str() {
        "mayhem_guide" => crate::guide::read(
            &root,
            args["championId"]
                .as_u64()
                .filter(|id| *id > 0 && *id < 10000)
                .ok_or("Invalid champion")?,
            args["forceRefresh"] == true,
            fixture,
        ),
        "mayhem_catalogue" => crate::guide::catalogue(&root, fixture),
        _ => Err("Unknown guide request".into()),
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(match response {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":{"detail":error}}),
    })
}
