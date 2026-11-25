mod capture;
mod controller;
mod data;
mod gamebar;
mod gpu;
mod recognition;
mod window;

use crate::shell::Desktop;
use capture::Area;
use data::{Catalogue, Stats};
use serde_json::json;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowLongPtrW, WindowFromPoint, GWL_EXSTYLE, WS_EX_NOACTIVATE,
        WS_EX_TRANSPARENT,
    },
};

pub fn setup() -> Result<(), String> {
    gamebar::activate()
}

pub fn preview(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("overlay-preview") {
        w.show().map_err(|e| e.to_string())?;
        return focus_preview(&w);
    }
    let w = WebviewWindowBuilder::new(
        app,
        "overlay-preview",
        WebviewUrl::App("overlay-preview.html".into()),
    )
    .title("Oracle · Augment overlay preview")
    .inner_size(1100., 700.)
    .min_inner_size(800., 600.)
    .center()
    .visible(false)
    .focused(false)
    .background_color(crate::match_guide::GROUND)
    .disable_drag_drop_handler()
    .on_navigation(|u| {
        (u.host_str() == Some("tauri.localhost") || u.scheme() == "tauri")
            && u.path() == "/overlay-preview.html"
    })
    .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
    .on_download(|_, _| false)
    .data_directory(app.state::<Desktop>().root.join("Dashboard-WebView2"))
    .build()
    .map_err(|e| e.to_string())?;
    w.show().map_err(|e| e.to_string())?;
    focus_preview(&w)?;
    Ok(())
}

fn focus_preview(w: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = w.hwnd().map_err(|e| e.to_string())?.0;
    if crate::switcher::focus(hwnd) {
        Ok(())
    } else {
        Err("Windows did not bring the preview to the foreground".into())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Target {
    hwnd: usize,
    area: Area,
    champion: u64,
    preview: bool,
    arena: bool,
    gpu: bool,
    gamebar: bool,
}
struct Scan {
    target: Target,
    started: Instant,
    result: Result<(Vec<recognition::Text>, i32, i32), String>,
}
fn target(app: &AppHandle) -> Option<Target> {
    let state = app.state::<Desktop>();
    if let Some(w) = app
        .get_webview_window("overlay-preview")
        .filter(|w| w.is_visible().unwrap_or(false))
    {
        let hwnd = w.hwnd().ok()?.0 as HWND;
        if unsafe { GetForegroundWindow() } == hwnd {
            return Some(Target {
                hwnd: hwnd as usize,
                area: capture::area(hwnd)?,
                champion: 1,
                preview: true,
                arena: false,
                gpu: state.preview && state.view.lock().ok()?["overlayGpuPreview"] == true,
                gamebar: false,
            });
        }
    }
    if state.isolated {
        if state.preview {
            let v = state.view.lock().ok()?;
            if v["overlayFixtureWindow"] == true {
                let hwnd = unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
                        std::ptr::null(),
                        windows_sys::core::w!("Oracle fullscreen overlay test"),
                    )
                };
                let mut pid = 0;
                unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                        hwnd, &mut pid,
                    );
                }
                let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()?
                    .join(".test");
                if !hwnd.is_null()
                    && unsafe { GetForegroundWindow() } == hwnd
                    && crate::switcher::image_path(pid).is_some_and(|p| {
                        p.starts_with(&workspace)
                            && p.file_name()
                                .is_some_and(|n| n == "oracle-overlay-fixture.exe")
                    })
                {
                    return Some(Target {
                        hwnd: hwnd as usize,
                        area: capture::area(hwnd)?,
                        champion: 1,
                        preview: true,
                        arena: v["overlayFixtureArena"] == true,
                        gpu: true,
                        gamebar: true,
                    });
                }
            }
        }
        return None;
    }
    let (enabled, champion, mode, phase) = {
        let v = state.view.lock().ok()?;
        (
            v["settings"]["overlayEnabled"] == true,
            v["guideTarget"]["id"].as_u64(),
            v["guideTarget"]["mode"].as_str().map(str::to_owned),
            v["phase"] == "InProgress",
        )
    };
    if !enabled || !matches!(mode.as_deref(), Some("mayhem" | "arena")) || !phase {
        return None;
    }
    let hwnd = crate::switcher::game_window();
    if hwnd.is_null() || unsafe { GetForegroundWindow() } != hwnd {
        return None;
    }
    Some(Target {
        hwnd: hwnd as usize,
        area: capture::area(hwnd)?,
        champion: champion?,
        preview: false,
        arena: mode.as_deref() == Some("arena"),
        gpu: true,
        gamebar: true,
    })
}
fn status(
    app: &AppHandle,
    kind: &str,
    message: &str,
    labels: &[window::Label],
    overlay: usize,
    target: Option<Target>,
) {
    let state = app.state::<Desktop>();
    let mut value = json!({"status":kind,"message":message,"visible":!labels.is_empty()});
    if state.preview {
        value["hwnd"] = json!(overlay);
        value["target"] = json!(target.map(|t| t.hwnd));
        value["labels"] = json!(labels
            .iter()
            .map(|l| json!({"id":l.id,"x":l.x,"y":l.y,"text":l.text}))
            .collect::<Vec<_>>());
        value["origin"] = target
            .map(|t| json!({"x":t.area.x,"y":t.area.y}))
            .unwrap_or_default();
        value["bounds"] = target
            .map(|t| json!({"width":t.area.w,"height":t.area.h}))
            .unwrap_or_default();
    }
    let mut v = state.view.lock().unwrap();
    if v["overlay"] != value {
        v["overlay"] = value.clone();
        drop(v);
        crate::support::event(&state.root, "overlay-state", value);
    }
}

pub fn inspect(app: &AppHandle) -> serde_json::Value {
    let state = app.state::<Desktop>();
    if !state.preview {
        return json!({});
    }
    let v = state.view.lock().unwrap();
    let o = &v["overlay"];
    let hwnd = o["hwnd"].as_u64().unwrap_or(0) as HWND;
    if hwnd.is_null() {
        return json!({});
    }
    unsafe {
        let styles = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let point = windows_sys::Win32::Foundation::POINT {
            x: (o["origin"]["x"].as_i64().unwrap_or(0) + o["labels"][0]["x"].as_i64().unwrap_or(0))
                as i32,
            y: (o["origin"]["y"].as_i64().unwrap_or(0)
                + o["labels"][0]["y"].as_i64().unwrap_or(0)
                + 8) as i32,
        };
        let mut rect = windows_sys::Win32::Foundation::RECT::default();
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect);
        let dc = windows_sys::Win32::Graphics::Gdi::GetDC(hwnd);
        let color = windows_sys::Win32::Graphics::Gdi::GetPixel(
            dc,
            (o["labels"][0]["x"].as_i64().unwrap_or(0) as i32
                - (rect.right - rect.left) * 19 / 200
                + 8)
            .max(0),
            o["labels"][0]["y"].as_i64().unwrap_or(0) as i32 + 8,
        );
        windows_sys::Win32::Graphics::Gdi::ReleaseDC(hwnd, dc);
        json!({"clickThrough":styles&WS_EX_TRANSPARENT!=0,"noActivate":styles&WS_EX_NOACTIVATE!=0,"hitOverlay":WindowFromPoint(point)==hwnd,"foregroundIsTarget":GetForegroundWindow() as usize==o["target"].as_u64().unwrap_or(0) as usize,"visible":windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd)!=0,"rect":{"x":rect.left,"y":rect.top,"width":rect.right-rect.left,"height":rect.bottom-rect.top},"backgroundPixel":color})
    }
}

pub use controller::watch;
