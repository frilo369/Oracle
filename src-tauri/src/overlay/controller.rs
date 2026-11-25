use super::*;
#[derive(Clone, Copy, PartialEq, Eq)]
struct Session {
    champion: u64,
    arena: bool,
    preview: bool,
}
fn session(t: Target) -> Session {
    Session {
        champion: t.champion,
        arena: t.arena,
        preview: t.preview,
    }
}
fn preparing(app: &AppHandle, current: Option<Target>) -> Option<Session> {
    if let Some(t) = current {
        return Some(session(t));
    }
    let state = app.state::<Desktop>();
    if state.isolated {
        return None;
    }
    let v = state.view.lock().ok()?;
    if v["settings"]["overlayEnabled"] != true
        || !matches!(
            v["phase"].as_str(),
            Some("ChampSelect" | "GameStart" | "InProgress" | "Reconnect")
        )
    {
        return None;
    }
    Some(Session {
        champion: v["guideTarget"]["id"].as_u64()?,
        arena: match v["guideTarget"]["mode"].as_str()? {
            "arena" => true,
            "mayhem" => false,
            _ => return None,
        },
        preview: false,
    })
}
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        unsafe {
            windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
                windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            );
        }
        let state = app.state::<Desktop>();
        let mayhem = Catalogue::bundled();
        let arena = Catalogue::for_mode(true);
        let (jobs, requests) = mpsc::sync_channel::<Option<Target>>(1);
        let (reply, scans) = mpsc::sync_channel::<Scan>(1);
        let evidence = state.preview.then(|| state.root.join("gpu-frame.bmp"));
        std::thread::spawn(move || {
            unsafe {
                windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
                    windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                );
            }
            let com = unsafe {
                windows::Win32::System::WinRT::RoInitialize(
                    windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
                )
            };
            let mut scanner: Option<capture::Scanner> = None;
            loop {
                let t = match requests.recv_timeout(Duration::from_millis(400)) {
                    Ok(Some(t)) => t,
                    Ok(None) | Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Some(s) = scanner.as_mut() {
                            s.pause();
                        }
                        continue;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let started = Instant::now();
                let result = if com.is_err() {
                    Err("Windows text recognition could not start".into())
                } else {
                    if scanner.is_none() {
                        scanner = capture::Scanner::new().ok();
                    }
                    match scanner.as_mut() {
                        Some(s) => s.read(t.hwnd as HWND, t.area, t.gpu, evidence.as_deref()),
                        None => Err("Install English OCR in Windows language options".into()),
                    }
                };
                if reply
                    .send(Scan {
                        target: t,
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
        let (stats_jobs, stats_requests) = mpsc::sync_channel::<Session>(1);
        let (stats_send, stats_results) = mpsc::sync_channel::<(Session, Result<Stats, String>)>(1);
        let handle = app.clone();
        let root = state.root.clone();
        std::thread::spawn(move || {
            while let Ok(s) = stats_requests.recv() {
                let result = if s.arena && s.preview {
                    Stats::arena(
                        &serde_json::from_str(include_str!(
                            "../../../tests/fixtures/overlay-arena.json"
                        ))
                        .unwrap(),
                        s.champion,
                        &Catalogue::for_mode(true),
                    )
                } else if s.arena {
                    crate::match_guide::arena_overlay_data(&handle, s.champion)
                        .and_then(|r| Stats::arena(&r, s.champion, &Catalogue::for_mode(true)))
                } else {
                    crate::guide::read(&root, s.champion, false, s.preview)
                        .and_then(|r| Stats::parse(&r, s.champion))
                };
                if stats_send.send((s, result)).is_err() {
                    break;
                }
            }
        });
        let mut current = None;
        let mut prepared = None;
        let mut stats: Option<(Session, Stats)> = None;
        let mut fetching = None;
        let mut next_stats = Instant::now();
        let mut stats_at = Instant::now();
        let mut next_scan = Instant::now();
        let mut expires = Instant::now();
        let mut pending = false;
        let mut stable = recognition::Stable::default();
        let mut labels = Vec::new();
        let mut failure = None;
        let mut data_failure = None;
        let mut surface = None::<window::Overlay>;
        let mut bridge = None;
        let mut bridge_check = Instant::now();
        let mut bridge_at = Instant::now();
        let mut bridge_visible = false;
        let mut wake_at = Instant::now();
        let mut wake_attempts = 0;
        while !state.is_closing() {
            window::Overlay::pump();
            let now = Instant::now();
            if now >= bridge_check {
                bridge = gamebar::folder(&state.root, state.isolated);
                bridge_check = now + Duration::from_secs(2);
            }
            let wanted = target(&app);
            let preparing = preparing(&app, wanted);
            if preparing != prepared {
                prepared = preparing;
                wake_attempts = 0;
                wake_at = now;
                next_stats = now;
                data_failure = None;
            }
            if wanted != current {
                current = wanted;
                if current.is_none() {
                    let _ = jobs.try_send(None);
                }
                labels.clear();
                stable.reset();
                failure = None;
                next_scan = now + Duration::from_millis(50);
                if let Some(w) = &surface {
                    w.hide();
                }
            }
            if let Some(s) = prepared {
                if fetching.is_none()
                    && now >= next_stats
                    && (stats_at.elapsed() > Duration::from_secs(if s.arena { 900 } else { 3600 })
                        || stats.as_ref().is_none_or(|(key, _)| *key != s))
                    && stats_jobs.try_send(s).is_ok()
                {
                    fetching = Some(s);
                }
            }
            while let Ok((s, result)) = stats_results.try_recv() {
                fetching = None;
                if prepared == Some(s) {
                    match result {
                        Ok(data) => {
                            stats = Some((s, data));
                            stats_at = now;
                            data_failure = None;
                        }
                        Err(e) => {
                            data_failure = Some(e);
                            next_stats = now + Duration::from_secs(if s.arena { 3 } else { 30 });
                        }
                    }
                }
            }
            let (health, help) = gamebar::health(bridge.as_deref());
            if !state.isolated
                && prepared.is_some()
                && health != "ready"
                && wake_attempts < 3
                && now >= wake_at
                && gamebar::configured(bridge.as_deref())
            {
                let _ = gamebar::activate();
                wake_attempts += 1;
                wake_at = now + Duration::from_secs(30);
            }
            if let Some(t) = current {
                if !pending && now >= next_scan && (!t.gamebar || health == "ready") {
                    pending = jobs.try_send(Some(t)).is_ok();
                    next_scan = now + Duration::from_millis(if t.gpu { 100 } else { 500 });
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
                        let catalogue = if scan.target.arena { &arena } else { &mayhem };
                        let found = stable
                            .accept(recognition::detect(&lines, catalogue, w as f32, h as f32));
                        labels.clear();
                        if state.preview {
                            crate::support::event(
                                &state.root,
                                "overlay-ocr",
                                json!({"width":w,"height":h,"matchedChoices":found.as_ref().map(|c|c.each_ref().map(|c|c.id)),"milliseconds":scan.started.elapsed().as_millis(),"names":lines.iter().filter_map(|l|catalogue.names.get(&data::key(&l.text)).map(|id|json!({"id":id,"x":l.x,"y":l.y,"width":l.w,"height":l.h}))).collect::<Vec<_>>(),"prompt":lines.iter().any(|l|data::key(&l.text).contains("augment"))}),
                            );
                        }
                        if let (Some(found), Some((s, data))) = (found, stats.as_ref()) {
                            if *s == session(scan.target) {
                                labels = found
                                    .iter()
                                    .map(|c| window::Label {
                                        id: c.id,
                                        x: (c.x * scan.target.area.w as f32 / w as f32) as i32,
                                        y: (c.y * scan.target.area.h as f32 / h as f32) as i32,
                                        text: data.label(c.id),
                                    })
                                    .collect();
                                expires = now + Duration::from_millis(750);
                            }
                        }
                        failure = None;
                    }
                    Err(e) => {
                        labels.clear();
                        stable.reset();
                        failure = Some(e);
                        next_scan = now + Duration::from_millis(250);
                    }
                }
            }
            if now > expires || target(&app) != current {
                labels.clear();
            }
            let mut showing = false;
            if let Some(t) = current {
                if !t.gamebar {
                    if !labels.is_empty() && surface.is_none() {
                        surface = window::Overlay::new();
                    }
                    if let Some(w) = surface.as_mut() {
                        if !labels.is_empty() {
                            showing = w.show(t.area, labels.clone());
                        } else {
                            w.hide();
                        }
                    }
                } else {
                    if let Some(w) = &surface {
                        w.hide();
                    }
                    showing = health == "ready" && !labels.is_empty();
                }
            } else {
                if let Some(w) = &surface {
                    w.hide();
                }
            }
            if !showing {
                labels.clear();
            }
            let (kind, message) = if showing {
                (
                    "visible",
                    if current.is_some_and(|t| t.preview) {
                        "Preview · example Annie statistics"
                    } else {
                        "Experimental · Sending pick rates and tiers to Game Bar"
                    },
                )
            } else if !state.preview
                && state.view.lock().unwrap()["settings"]["overlayEnabled"] != true
            {
                ("off", "Experimental · Disabled by default")
            } else if current.is_none_or(|t| t.gamebar) && health != "ready" {
                ("setup", help)
            } else if let Some(error) = failure.as_deref().or(data_failure.as_deref()) {
                ("unavailable", error)
            } else if current.is_some() {
                (
                    "recognizing",
                    "Waiting for augment choices · Automatic detection active",
                )
            } else {
                ("idle", "Arena + Mayhem · Game Bar ready")
            };
            status(
                &app,
                kind,
                message,
                &labels,
                surface.as_ref().map_or(0, |w| w.hwnd as usize),
                current,
            );
            let live = current.filter(|t| t.gamebar);
            let publish_visible = live.is_some() && !labels.is_empty();
            if bridge_at.elapsed() >= Duration::from_millis(100)
                && (publish_visible
                    || bridge_visible
                    || bridge_at.elapsed() >= Duration::from_secs(2))
            {
                if let Some(path) = bridge.as_deref() {
                    let output = if live.is_some() {
                        labels.as_slice()
                    } else {
                        &[]
                    };
                    let _ = gamebar::publish(path, live.map(|t| t.area), output);
                }
                bridge_at = now;
                bridge_visible = publish_visible;
            }
            std::thread::sleep(Duration::from_millis(if current.is_some() {
                25
            } else {
                200
            }));
        }
        if let Some(path) = bridge {
            let _ = gamebar::publish(&path, None, &[]);
        }
    });
}
