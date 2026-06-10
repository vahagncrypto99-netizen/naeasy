//! Composition root: wires concrete infrastructure into the application
//! service, registers plugins and command handlers. No business logic here.

use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

mod app;
mod domain;
mod infra;
mod ui;

use app::state::AppState;
use app::workspace_service::WorkspaceService;
use infra::config_repository::{ConfigRepository, JsonConfigRepository};
use infra::shortcut::ShortcutService;

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
                        ui::tray::toggle_main(app);
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

            // Wire concrete platform implementations into the service (DI).
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            let config_path = config_dir.join("config.json");
            let repo: Arc<dyn ConfigRepository> =
                Arc::new(JsonConfigRepository::new(config_path));
            let service = WorkspaceService::new(
                repo,
                infra::ide_detector::platform_detector(),
                infra::project_launcher::platform_launcher(),
            );
            let shortcut_accel = service.shortcut();
            app.manage(AppState { service });

            // Register the global hotkey that toggles the window.
            if let Err(e) = ShortcutService::register(app.handle(), &shortcut_accel) {
                log::warn!("global shortcut '{shortcut_accel}' not registered: {e}");
            }

            // Build the tray icon (menu-bar entry).
            ui::tray::setup(app)?;

            // Show the window on first launch so the app is visibly "there".
            log::info!("showing main window on launch");
            ui::tray::show_main(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ui::commands::get_data,
            ui::commands::rescan,
            ui::commands::add_workspace,
            ui::commands::remove_workspace,
            ui::commands::detect_ides,
            ui::commands::add_ide,
            ui::commands::remove_ide,
            ui::commands::set_default_ide,
            ui::commands::open_project,
            ui::commands::reveal_in_finder,
            ui::commands::quit_app,
            ui::commands::set_shortcut,
            ui::commands::scan_open_projects,
            ui::commands::set_project_ide,
            ui::commands::set_open_in_tabs
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
        .run(|_app_handle, _event| {
            // Clicking the Dock icon when no window is visible reopens it.
            // `RunEvent::Reopen` exists only on macOS.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                ui::tray::show_main(_app_handle);
            }
        });
}
