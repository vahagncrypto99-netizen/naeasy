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
    let builder = tauri::Builder::default();
    // NSPanel support: a non-activating panel gets keyboard focus without
    // activating the app, so macOS never switches Spaces to "follow" it —
    // the Spotlight/Raycast approach.
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    // A second launch (app menu, terminal) must not raise a rival process —
    // it loses the race for the global hotkey and just sits there. Surface
    // the running instance instead. macOS re-uses the running bundle itself.
    #[cfg(not(target_os = "macos"))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
        // Fired from the plugin's D-Bus listener thread. GTK and Xlib may
        // only be touched from the main thread — calling straight into the
        // window here aborts the process ("xcb_xlib_threads_sequence_lost").
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || ui::tray::show_main(&handle));
    }));

    builder
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
                        // The hotkey listener runs on its own thread. GTK and
                        // Xlib may only be touched from the main one — driving
                        // the window straight from here aborts the process
                        // mid-toggle, and only sometimes, being a race.
                        #[cfg(not(target_os = "macos"))]
                        {
                            let handle = app.clone();
                            let _ = app.run_on_main_thread(move || {
                                ui::tray::toggle_main(&handle);
                            });
                        }
                        #[cfg(target_os = "macos")]
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
                Arc::new(infra::git::SystemGitClient),
            );
            let shortcut_accel = service.shortcut();
            app.manage(AppState {
                service,
                layout_provider: infra::keyboard_layouts::platform_layout_provider(),
                autohide_suppressed: std::sync::atomic::AtomicBool::new(false),
            });

            // Register the global hotkey that toggles the window.
            if let Err(e) = ShortcutService::register(app.handle(), &shortcut_accel) {
                log::warn!("global shortcut '{shortcut_accel}' not registered: {e}");
            }

            // Re-emit keyboard-layout maps to the frontend whenever the user
            // adds/removes/changes a layout. Registered on the main thread so
            // the notification callback runs there too.
            #[cfg(target_os = "macos")]
            infra::keyboard_layouts::watch_layout_changes(app.handle().clone());

            // Build the tray icon (menu-bar entry).
            ui::tray::setup(app)?;

            // Turn the main window into a Spotlight-style NSPanel: floating,
            // non-activating (keyboard without app activation — so macOS has
            // no reason to switch Spaces), visible on all Spaces including
            // fullscreen ones.
            #[cfg(target_os = "macos")]
            #[allow(deprecated)] // the plugin's API still takes the cocoa type
            {
                use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
                use tauri_nspanel::{panel_delegate, WebviewWindowExt as _};

                let w = app
                    .get_webview_window("main")
                    .expect("main window must exist");

                // Native translucent material under the UI (popover glass,
                // follows the system light/dark appearance). State must be
                // forced to Active: a non-activating panel is never "active"
                // in AppKit terms, and the default FollowsWindowActiveState
                // renders the material flat and opaque. Radius matches the
                // CSS .app border-radius.
                if let Err(e) = window_vibrancy::apply_vibrancy(
                    &w,
                    window_vibrancy::NSVisualEffectMaterial::Popover,
                    Some(window_vibrancy::NSVisualEffectState::Active),
                    Some(16.0),
                ) {
                    log::warn!("vibrancy not applied: {e}");
                }

                let panel = w.to_panel().expect("failed to convert window to panel");
                // NSFloatingWindowLevel — above normal windows, like Spotlight.
                panel.set_level(4);
                // NSWindowStyleMaskNonActivatingPanel
                panel.set_style_mask(1 << 7);
                panel.set_collection_behaviour(
                    NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
                );

                // Spotlight behavior: hide when the panel loses keyboard focus
                // (click outside, app switch). A floating non-activating panel
                // never hides by itself otherwise. Exception: focus moving to
                // another window of OUR app (folder-picker dialog) must not
                // close the popover — hence the delayed keyWindow check.
                let delegate = panel_delegate!(NaeasyPanelDelegate {
                    window_did_resign_key
                });
                let handle = app.handle().clone();
                delegate.set_listener(Box::new(move |name: String| {
                    if name != "window_did_resign_key" {
                        return;
                    }
                    let handle = handle.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        let h = handle.clone();
                        let _ = handle.run_on_main_thread(move || {
                            use objc2::MainThreadMarker;
                            use objc2_app_kit::NSApplication;
                            let Some(mtm) = MainThreadMarker::new() else {
                                return;
                            };
                            let ns_app = NSApplication::sharedApplication(mtm);
                            if ns_app.keyWindow().is_none() {
                                ui::tray::hide_main(&h);
                            }
                        });
                    });
                }));
                panel.set_delegate(delegate);
            }
            // Belt-and-suspenders for the helper TaoWindow (pinned to the
            // launch Space otherwise).
            ui::tray::make_windows_join_all_spaces(app.handle());

            // Restore the configured geometry (size mode, float position).
            ui::tray::apply_window_geometry(app.handle());

            // Take the keyboard every time the popover appears.
            #[cfg(not(target_os = "macos"))]
            ui::tray::arm_focus_on_map(app.handle());

            // Show the window on first launch so the app is visibly "there".
            log::info!("showing main window on launch");
            ui::tray::show_main(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ui::commands::get_data,
            ui::commands::get_layout_maps,
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
            ui::commands::set_open_in_tabs,
            ui::commands::open_repo_url,
            ui::commands::fast_start,
            ui::commands::add_base_branch,
            ui::commands::remove_base_branch,
            ui::commands::set_default_base_branch,
            ui::commands::hide_window,
            ui::commands::set_autohide_suppressed,
            ui::commands::open_last_mr,
            ui::commands::toggle_pin,
            ui::commands::set_section_prefs,
            ui::commands::set_window_prefs
        ])
        .on_window_event(|window, event| {
            // The red close button hides the window instead of quitting, so the
            // app keeps running. Reopen it from the Dock icon or menu-bar icon.
            if window.label() != "main" {
                return;
            }
            match event {
                // The red close button hides instead of quitting.
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    ui::tray::hide_main(window.app_handle());
                }
                // Spotlight behavior where there is no NSPanel delegate to
                // do it: hide once the focus leaves the popover. Debounced,
                // because Linux WMs drop focus for a moment while raising a
                // window — and skipped while a native dialog owns the focus.
                #[cfg(not(target_os = "macos"))]
                tauri::WindowEvent::Focused(false) => {
                    let app = window.app_handle().clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            let suppressed = handle
                                .state::<AppState>()
                                .autohide_suppressed
                                .load(std::sync::atomic::Ordering::Relaxed);
                            let refocused = handle
                                .get_webview_window("main")
                                .and_then(|w| w.is_focused().ok())
                                .unwrap_or(false);
                            if !suppressed && !refocused {
                                ui::tray::hide_main(&handle);
                            }
                        });
                    });
                }
                // Remember geometry in memory (persisted on hide). The
                // service itself ignores these in pinned/fixed modes.
                tauri::WindowEvent::Moved(pos) => {
                    let state = window.app_handle().state::<AppState>();
                    state.service.remember_window_pos(pos.x, pos.y);
                }
                tauri::WindowEvent::Resized(size) => {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let logical = size.to_logical::<f64>(scale);
                    let state = window.app_handle().state::<AppState>();
                    state
                        .service
                        .remember_window_size(logical.width as u32, logical.height as u32);
                }
                _ => {}
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
