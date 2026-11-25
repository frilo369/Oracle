mod capture;
mod data;
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
            });
        }
    }
    if state.isolated {
        return None;
    }
    let (enabled, champion, mayhem, phase) = {
        let v = state.view.lock().ok()?;
        (
            v["settings"]["overlayEnabled"] == true,
            v["guideTarget"]["id"].as_u64(),
            v["guideTarget"]["mode"] == "mayhem",
            v["phase"] == "InProgress",
        )
    };
    if !enabled || !mayhem || !phase {
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

pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let state = app.state::<Desktop>();
        let catalogue = Catalogue::bundled();
        let mut surface: Option<window::Overlay> = None;
        let (jobs, requests) = mpsc::sync_channel::<Target>(1);
        let (reply, scans) = mpsc::sync_channel::<Scan>(1);
        std::thread::spawn(move || {
            let com = unsafe {
                windows::Win32::System::WinRT::RoInitialize(
                    windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
                )
            };
            let mut scanner = None;
            while let Ok(target) = requests.recv() {
                let started = Instant::now();
                let result = if com.is_err() {
                    Err("Windows text recognition could not start".into())
                } else {
                    if scanner.is_none() {
                        scanner = capture::Scanner::new().ok();
                    }
                    match &scanner {
                        Some(s) => s.read(target.hwnd as HWND, target.area),
                        None => Err(
                            "Install English OCR in Windows language options to use the overlay"
                                .into(),
                        ),
                    }
                };
                if reply
                    .send(Scan {
                        target,
                        started,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
            drop(scanner);
            if com.is_ok() {
                unsafe {
                    windows::Win32::System::WinRT::RoUninitialize();
                }
            }
        });
        let (send_stats, stats_results) = mpsc::channel::<(u64, bool, Result<Stats, String>)>();
        let mut current = None;
        let mut stats = None;
        let mut fetching = None;
        let mut next_stats = Instant::now();
        let mut stats_at = Instant::now();
        let mut stable = recognition::Stable::default();
        let mut pending = false;
        let mut next_scan = Instant::now();
        let mut expires = Instant::now();
        let mut labels = Vec::new();
        let mut failure = None::<String>;
        let mut stats_failure = None::<String>;
        while !state.is_closing() {
            window::Overlay::pump();
            let now = Instant::now();
            let wanted = target(&app);
            if wanted != current {
                if wanted.map(|t| (t.champion, t.preview))
                    != current.map(|t: Target| (t.champion, t.preview))
                {
                    next_stats = now;
                    stats_failure = None;
                }
                current = wanted;
                labels.clear();
                stable.reset();
                next_scan = now + Duration::from_millis(250);
                failure = None;
                if let Some(w) = &surface {
                    w.hide();
                }
            }
            if let Some(t) = current {
                if fetching != Some((t.champion, t.preview))
                    && now >= next_stats
                    && (stats_at.elapsed() >= Duration::from_secs(3600)
                        || stats.as_ref().is_none_or(|(id, preview, _)| {
                            (*id, *preview) != (t.champion, t.preview)
                        }))
                {
                    stats = None;
                    fetching = Some((t.champion, t.preview));
                    let root = state.root.clone();
                    let sender = send_stats.clone();
                    std::thread::spawn(move || {
                        let result = crate::guide::read(&root, t.champion, false, t.preview)
                            .and_then(|r| Stats::parse(&r, t.champion));
                        let _ = sender.send((t.champion, t.preview, result));
                    });
                }
                if !pending && now >= next_scan {
                    if jobs.try_send(t).is_ok() {
                        pending = true;
                    }
                    next_scan = now + Duration::from_millis(500);
                }
            }
            while let Ok((id, preview, result)) = stats_results.try_recv() {
                if fetching == Some((id, preview)) {
                    fetching = None;
                }
                if current.is_some_and(|t| (t.champion, t.preview) == (id, preview)) {
                    match result {
                        Ok(data) => {
                            stats = Some((id, preview, data));
                            stats_at = now;
                            stats_failure = None;
                            next_stats = now;
                        }
                        Err(_) => {
                            stats_failure =
                                Some("Augment data unavailable · Retrying automatically".into());
                            next_stats = now + Duration::from_secs(30);
                        }
                    }
                }
            }
            while let Ok(scan) = scans.try_recv() {
                pending = false;
                if Some(scan.target) != current
                    || scan.started.elapsed() > Duration::from_secs(1)
                    || target(&app) != current
                {
                    continue;
                }
                match scan.result {
                    Ok((lines, w, h)) => {
                        if state.preview {
                            crate::support::event(
                                &state.root,
                                "overlay-ocr",
                                json!({"width":w,"height":h,"matches":lines.iter().filter_map(|l|catalogue.names.get(&data::key(&l.text)).map(|id|json!({"id":id,"x":l.x,"y":l.y,"w":l.w,"h":l.h}))).collect::<Vec<_>>(),"prompt":lines.iter().any(|l|data::key(&l.text).contains("chooseanaugment"))}),
                            );
                        }
                        let found = stable
                            .accept(recognition::detect(&lines, &catalogue, w as f32, h as f32));
                        labels.clear();
                        if let (Some(found), Some((id, preview, data))) = (found, stats.as_ref()) {
                            if (*id, *preview) == (scan.target.champion, scan.target.preview) {
                                labels = found
                                    .iter()
                                    .map(|c| window::Label {
                                        id: c.id,
                                        x: (c.x * scan.target.area.w as f32 / w as f32) as i32,
                                        y: (c.y * scan.target.area.h as f32 / h as f32) as i32,
                                        text: data.label(c.id),
                                    })
                                    .collect();
                                expires = now + Duration::from_millis(1100);
                            }
                        }
                        failure = None;
                    }
                    Err(message) => {
                        labels.clear();
                        stable.reset();
                        failure = Some(message);
                    }
                }
            }
            if now > expires {
                labels.clear();
            }
            if target(&app) != current {
                labels.clear();
            }
            if !labels.is_empty() && surface.is_none() {
                surface = window::Overlay::new();
                if surface.is_none() {
                    labels.clear();
                    failure = Some("Could not create augment overlay".into());
                }
            }
            if let Some(t) = current.filter(|_| !labels.is_empty()) {
                if target(&app) == Some(t) {
                    if let Some(w) = surface.as_mut() {
                        if w.show(t.area, labels.clone()) {
                            status(
                                &app,
                                "visible",
                                if t.preview {
                                    "Preview · example Annie statistics"
                                } else {
                                    "Showing Mayhem augment pick rates and tiers"
                                },
                                &labels,
                                w.hwnd as usize,
                                Some(t),
                            );
                        } else {
                            w.hide();
                            labels.clear();
                            failure = Some("Could not position augment overlay".into());
                        }
                    }
                }
            } else {
                if let Some(w) = &surface {
                    w.hide();
                }
                status(
                    &app,
                    if failure.is_some() || stats_failure.is_some() {
                        "unavailable"
                    } else {
                        "idle"
                    },
                    failure.as_deref().or(stats_failure.as_deref()).unwrap_or(
                        if current.is_some() {
                            "Waiting for three recognizable augment choices"
                        } else {
                            "Mayhem · Borderless fullscreen"
                        },
                    ),
                    &[],
                    surface.as_ref().map_or(0, |w| w.hwnd as usize),
                    current,
                );
            }
            std::thread::sleep(Duration::from_millis(if current.is_some() {
                75
            } else {
                250
            }));
        }
    });
}
