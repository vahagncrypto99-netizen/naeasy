use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

mod domain;
mod infra;

use domain::models::{default_shortcut, now_secs, AppData, Config, Ide, Recent, Workspace};
use domain::tree::build_app_data;
use infra::config_repository::{ConfigRepository, JsonConfigRepository};
use infra::ide_detector::{ide_name_from_path, IdeDetector};
use infra::project_launcher::ProjectLauncher;
use infra::shortcut::ShortcutService;

struct AppState {
    config: Mutex<Config>,
    repo: Arc<dyn ConfigRepository>,
    detector: Arc<dyn IdeDetector>,
    launcher: Arc<dyn ProjectLauncher>,
}

fn gen_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}", nanos, n)
}

// ------------------------- Commands -------------------------

#[tauri::command]
fn get_data(state: State<AppState>) -> AppData {
    let config = state.config.lock().unwrap();
    build_app_data(&config)
}

#[tauri::command]
fn rescan(state: State<AppState>) -> AppData {
    let config = state.config.lock().unwrap();
    build_app_data(&config)
}

#[tauri::command]
fn add_workspace(state: State<AppState>, path: String) -> Result<AppData, String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }
    let mut config = state.config.lock().unwrap();
    if config.workspaces.iter().any(|w| w.path == path) {
        return Err("Workspace already added".into());
    }
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    config.workspaces.push(Workspace {
        id: gen_id(),
        name,
        path,
    });
    state.repo.save(&config)?;
    Ok(build_app_data(&config))
}

#[tauri::command]
fn remove_workspace(state: State<AppState>, id: String) -> Result<AppData, String> {
    let mut config = state.config.lock().unwrap();
    config.workspaces.retain(|w| w.id != id);
    state.repo.save(&config)?;
    Ok(build_app_data(&config))
}

#[tauri::command]
fn detect_ides(state: State<AppState>) -> Result<AppData, String> {
    let detected = state.detector.detect();
    let mut config = state.config.lock().unwrap();
    for ide in detected {
        if !config.ides.iter().any(|i| i.path == ide.path) {
            config.ides.push(ide);
        }
    }
    if config.default_ide_id.is_none() {
        config.default_ide_id = config.ides.first().map(|i| i.id.clone());
    }
    state.repo.save(&config)?;
    Ok(build_app_data(&config))
}

#[tauri::command]
fn add_ide(state: State<AppState>, path: String) -> Result<AppData, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Path does not exist: {path}"));
    }
    let mut config = state.config.lock().unwrap();
    if config.ides.iter().any(|i| i.path == path) {
        return Err("IDE already added".into());
    }
    let name = ide_name_from_path(&path);
    let ide = Ide {
        id: gen_id(),
        name,
        path,
    };
    let new_id = ide.id.clone();
    config.ides.push(ide);
    if config.default_ide_id.is_none() {
        config.default_ide_id = Some(new_id);
    }
    state.repo.save(&config)?;
    Ok(build_app_data(&config))
}

#[tauri::command]
fn remove_ide(state: State<AppState>, id: String) -> Result<AppData, String> {
    let mut config = state.config.lock().unwrap();
    config.ides.retain(|i| i.id != id);
    if config.default_ide_id.as_deref() == Some(id.as_str()) {
        config.default_ide_id = config.ides.first().map(|i| i.id.clone());
    }
    state.repo.save(&config)?;
    Ok(build_app_data(&config))
}

#[tauri::command]
fn set_default_ide(state: State<AppState>, id: String) -> Result<AppData, String> {
    let mut config = state.config.lock().unwrap();
    if !config.ides.iter().any(|i| i.id == id) {
        return Err("Unknown IDE".into());
    }
    config.default_ide_id = Some(id);
    state.repo.save(&config)?;
    Ok(build_app_data(&config))
}

/// Open a project folder in the chosen (or default) IDE.
#[tauri::command]
fn open_project(
    state: State<AppState>,
    project_path: String,
    ide_id: Option<String>,
) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    let chosen = ide_id.or_else(|| config.default_ide_id.clone());
    let ide = match chosen.and_then(|id| config.ides.iter().find(|i| i.id == id).cloned()) {
        Some(ide) => ide,
        None => return Err("No IDE selected. Add an IDE first.".into()),
    };

    if !Path::new(&project_path).exists() {
        return Err(format!("Project not found: {project_path}"));
    }

    state.launcher.open(&ide.path, &project_path)?;

    // Remember when and with which IDE this project was opened.
    config.recents.insert(
        project_path.clone(),
        Recent {
            last_opened: now_secs(),
            ide: ide.name.clone(),
        },
    );
    let _ = state.repo.save(&config);
    Ok(())
}

/// Reveal the project folder in Finder.
#[tauri::command]
fn reveal_in_finder(state: State<AppState>, path: String) -> Result<(), String> {
    state.launcher.reveal(&path)
}

// ------------------------- Open-project detection -------------------------

/// Which of the given project folder names are currently open in an IDE.
/// Returns a map of name -> IDE process name. Best-effort (needs Accessibility).
#[tauri::command]
fn scan_open_projects(
    state: State<AppState>,
    names: Vec<String>,
) -> std::collections::HashMap<String, String> {
    state.launcher.scan_open(&names)
}

// ------------------------- Global shortcut -------------------------

fn toggle_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let minimized = w.is_minimized().unwrap_or(false);
        let visible = w.is_visible().unwrap_or(false);
        if visible && !minimized {
            let _ = w.hide();
        } else {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

#[tauri::command]
fn set_shortcut(
    app: tauri::AppHandle,
    state: State<AppState>,
    accel: String,
) -> Result<AppData, String> {
    ShortcutService::register(&app, &accel)?;
    let mut config = state.config.lock().unwrap();
    config.shortcut = Some(accel);
    state.repo.save(&config)?;
    Ok(build_app_data(&config))
}

// ------------------------- Tray + window -------------------------

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("naeasy".into()),
                    },
                ))
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        toggle_main(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            log::info!("naeasy starting up");
            // Pure menu-bar app: no Dock icon. Reach it via the menu-bar icon
            // or the global hotkey.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Resolve config path and load config into state.
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            let config_path = config_dir.join("config.json");
            let repo: Arc<dyn ConfigRepository> =
                Arc::new(JsonConfigRepository::new(config_path));
            let config = repo.load();
            let shortcut_accel = config.shortcut.clone().unwrap_or_else(default_shortcut);
            app.manage(AppState {
                config: Mutex::new(config),
                repo,
                detector: infra::ide_detector::platform_detector(),
                launcher: infra::project_launcher::platform_launcher(),
            });

            // Register the global hotkey that toggles the window.
            if let Err(e) = ShortcutService::register(app.handle(), &shortcut_accel) {
                log::warn!("global shortcut '{shortcut_accel}' not registered: {e}");
            }

            // Build the tray icon (menu-bar entry).
            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

            let quit = MenuItemBuilder::with_id("quit", "Quit naeasy").build(app)?;
            let show = MenuItemBuilder::with_id("show", "Open naeasy").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            // Dedicated monochrome template icon for the menu bar — tints to
            // the light/dark menu bar automatically (like JetBrains Toolbox).
            // Fall back to the app icon if the tray PNG can't be decoded, so a
            // bad icon never crashes startup.
            let tray_builder = TrayIconBuilder::with_id("main-tray");
            let tray_builder = match tauri::image::Image::from_bytes(include_bytes!(
                "../icons/tray.png"
            )) {
                Ok(img) => tray_builder.icon(img).icon_as_template(true),
                Err(_) => tray_builder.icon(app.default_window_icon().unwrap().clone()),
            };

            let _tray = tray_builder
                .tooltip("naeasy — project navigator")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "show" => show_main(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let visible = window.is_visible().unwrap_or(false);
                            log::info!("tray clicked; window visible = {visible}");
                            if visible {
                                let _ = window.hide();
                            } else {
                                position_under_tray(&window, &rect);
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Show the window on first launch so the app is visibly "there".
            log::info!("showing main window on launch");
            show_main(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_data,
            rescan,
            add_workspace,
            remove_workspace,
            detect_ides,
            add_ide,
            remove_ide,
            set_default_ide,
            open_project,
            reveal_in_finder,
            quit_app,
            set_shortcut,
            scan_open_projects
        ])
        .on_window_event(|window, event| {
            // The red close button hides the window instead of quitting, so the
            // app keeps running. Reopen it from the Dock icon or menu-bar icon.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building naeasy")
        .run(|app_handle, event| {
            // Clicking the Dock icon when no window is visible reopens it.
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main(app_handle);
            }
        });
}

/// Show, un-minimize and focus the main window.
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Quit the whole app (called from the UI "Quit" button).
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Position the popover window horizontally centered under the tray icon,
/// clamped to the monitor so it can never end up off-screen.
fn position_under_tray(window: &tauri::WebviewWindow, rect: &tauri::Rect) {
    use tauri::{PhysicalPosition, Position};

    let win_w = match window.outer_size() {
        Ok(s) => s.width as f64,
        Err(_) => return,
    };
    let sf = window.scale_factor().unwrap_or(1.0);

    // Tray rect — convert to physical pixels.
    let tray_x = match rect.position {
        Position::Physical(p) => p.x as f64,
        Position::Logical(p) => p.x * sf,
    };
    let tray_w = match rect.size {
        tauri::Size::Physical(s) => s.width as f64,
        tauri::Size::Logical(s) => s.width * sf,
    };

    // Monitor bounds (physical). Fall back to centering if unknown.
    let (mon_x, mon_y, mon_w) = match window.primary_monitor() {
        Ok(Some(m)) => {
            let p = m.position();
            let s = m.size();
            (p.x as f64, p.y as f64, s.width as f64)
        }
        _ => {
            let _ = window.center();
            return;
        }
    };

    let mut x = tray_x + tray_w / 2.0 - win_w / 2.0;
    let min_x = mon_x + 8.0;
    let max_x = mon_x + mon_w - win_w - 8.0;
    if x < min_x {
        x = min_x;
    }
    if max_x > min_x && x > max_x {
        x = max_x;
    }
    // Just below the menu bar (~24pt).
    let y = mon_y + (26.0 * sf);

    let _ = window.set_position(Position::Physical(PhysicalPosition {
        x: x as i32,
        y: y as i32,
    }));
}

