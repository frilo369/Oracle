#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod appearance;
mod asset_wad;
mod errorlog;
mod guide;
mod guide_assets;
mod guide_keys;
mod league;
mod league_connection;
mod league_path;
mod match_guide;
mod procs;
mod settings;
mod shell;
mod support;
mod switcher;
mod update;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
fn main() {
    let mut context = tauri::generate_context!();
    // Disposable QA gets its own instance identity, so it cannot forward commands to the
    // installed app or touch the player's running session. Forwarded QA launches use this too.
    let qa_root = std::env::var_os("ORACLE_DATA_DIR")
        .map(std::path::PathBuf::from)
        .and_then(|p| p.canonicalize().ok());
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let qa = qa_root.as_ref().is_some_and(|p| {
        workspace
            .join(".test")
            .canonicalize()
            .is_ok_and(|allowed| p.starts_with(allowed))
    }) && std::env::current_exe().ok().is_some_and(|p| {
        p.starts_with(workspace.join("target")) || p.starts_with(workspace.join(".test"))
    });
    if std::env::args().any(|a| a == "--fixture") && !qa {
        return;
    }
    if qa {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        qa_root
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase()
            .hash(&mut hash);
        context.config_mut().identifier = format!("com.frilo369.oracle.qa{:x}", hash.finish());
    }
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if args.iter().any(|a| a == "--quit") {
                app.exit(0);
            } else if !args.iter().any(|a| a == "--tray") {
                show_from_event(app);
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .register_asynchronous_uri_scheme_protocol("umbra-icon", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let label = ctx.webview_label().to_owned();
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(guide_assets::response(&app, &label, &request))
            });
        })
        .invoke_handler(tauri::generate_handler![
            shell::ready,
            shell::action,
            shell::guide_data
        ])
        .setup(move |app| {
            let root = std::env::var_os("ORACLE_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or(app.path().app_local_data_dir()?);
            std::fs::create_dir_all(&root)?;
            let preview = std::env::args().any(|a| a == "--fixture");
            if preview {
                std::fs::write(root.join("fixture-mode"), b"")?;
            }
            let state = shell::Desktop::new(root, preview, qa);
            let quiet = std::env::args().any(|a| a == "--tray")
                || state.view.lock().unwrap()["settings"]["launchInTray"] == true;
            app.manage(state);
            if std::env::args().any(|a| a == "--quit") {
                app.handle().exit(0);
                return Ok(());
            }
            let open = MenuItem::with_id(app, "open", "Open Oracle", true, None::<&str>)?;
            let guide = MenuItem::with_id(app, "guide", "Open current guide", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Oracle", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &guide, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Oracle · League companion")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_from_event(app),
                    "guide" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            if let Err(e) = match_guide::open(&app) {
                                shell::report(&app, &e);
                            }
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_from_event(tray.app_handle());
                    }
                })
                .build(app)?;
            if !quiet {
                shell::show_dashboard(app.handle())?;
            }
            league::watch(app.handle().clone());
            switcher::watch(app.handle().clone());
            update::startup(app.handle().clone());
            Ok(())
        })
        .build(context);
    match result {
        Ok(app) => app.run(|app, event| match event {
            tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } => api.prevent_exit(),
            tauri::RunEvent::Exit => {
                app.state::<shell::Desktop>()
                    .closing
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            _ => {}
        }),
        Err(error) => {
            let root = std::env::var_os("ORACLE_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::var_os("LOCALAPPDATA")
                        .map(std::path::PathBuf::from)
                        .unwrap_or_default()
                        .join("com.frilo369.oracle")
                });
            let _ = std::fs::create_dir_all(&root);
            errorlog::note(&root, "startup", &error.to_string());
        }
    }
}
fn show_from_event(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = shell::show_dashboard(&app) {
            errorlog::note(&app.state::<shell::Desktop>().root, "dashboard", &error);
        }
    });
}
