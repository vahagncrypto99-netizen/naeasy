//! Tray icon (menu-bar entry) and main-window show/hide/position plumbing.

use tauri::{Emitter, Manager};

/// Tell the frontend the window was just shown — it resets transient UI
/// (open settings panel, popovers) so the user always lands on the tree.
fn emit_shown(app: &tauri::AppHandle) {
    let _ = app.emit("naeasy://shown", ());
}

/// Emitted right after the window hides; the frontend resets transient UI
/// while invisible, so the next show never flashes a stale settings panel.
fn emit_hidden(app: &tauri::AppHandle) {
    let _ = app.emit("naeasy://hidden", ());
}

/// Make every normal-level window of the app join all Spaces and fullscreen
/// Spaces (the status-bar window and other high-level windows are left
/// alone). Without this the helper TaoWindow stays pinned to the launch Space
/// and macOS switches to it whenever the app is activated. The webview's own
/// NSWindow registers in `NSApp.windows` late, so this is re-applied on every
/// show — it's a handful of cheap setter calls.
#[cfg(target_os = "macos")]
pub fn make_windows_join_all_spaces(app: &tauri::AppHandle) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSWindow, NSWindowCollectionBehavior};

    const BEHAVIOR: NSWindowCollectionBehavior = NSWindowCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces.0
            | NSWindowCollectionBehavior::FullScreenAuxiliary.0,
    );

    // The main webview window — via its handle (always reachable).
    if let Some(w) = app.get_webview_window("main") {
        if let Ok(ptr) = w.ns_window() {
            let ns_window = unsafe { &*(ptr as *const NSWindow) };
            ns_window.setCollectionBehavior(BEHAVIOR);
        }
    }
    // Every other registered normal-level window (e.g. the helper TaoWindow).
    if let Some(mtm) = MainThreadMarker::new() {
        let ns_app = NSApplication::sharedApplication(mtm);
        for w in ns_app.windows().iter() {
            if w.level() == 0 {
                w.setCollectionBehavior(BEHAVIOR);
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn make_windows_join_all_spaces(_app: &tauri::AppHandle) {}

/// Anchor the main window under the tray icon (like every menu-bar app).
/// Used by all show paths so the hotkey and tray click behave the same.
fn position_main_under_tray(app: &tauri::AppHandle) {
    if let (Some(tray), Some(window)) = (
        app.tray_by_id("main-tray"),
        app.get_webview_window("main"),
    ) {
        if let Ok(Some(rect)) = tray.rect() {
            position_under_tray(&window, &rect);
        }
    }
}

/// Hide the main window (panel-aware). The single hide path for every
/// trigger — hotkey toggle, tray click, auto-hide, frontend, close button.
pub fn hide_main(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt as _;
        if let Ok(panel) = app.get_webview_panel("main") {
            panel.order_out(None);
            emit_hidden(app);
            return;
        }
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
        emit_hidden(app);
    }
}

/// Toggle the main window from the global hotkey.
pub fn toggle_main(app: &tauri::AppHandle) {
    // macOS: drive the NSPanel directly — `show()` makes it key WITHOUT
    // activating the app, so the current Space stays put.
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt as _;
        if let Ok(panel) = app.get_webview_panel("main") {
            if panel.is_visible() {
                hide_main(app);
            } else {
                make_windows_join_all_spaces(app);
                position_main_under_tray(app);
                panel.show();
                emit_shown(app);
            }
            return;
        }
    }
    if let Some(w) = app.get_webview_window("main") {
        let minimized = w.is_minimized().unwrap_or(false);
        let visible = w.is_visible().unwrap_or(false);
        if visible && !minimized {
            hide_main(app);
        } else {
            make_windows_join_all_spaces(app);
            position_main_under_tray(app);
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
            emit_shown(app);
        }
    }
}

/// Show, un-minimize and focus the main window.
pub fn show_main(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt as _;
        if let Ok(panel) = app.get_webview_panel("main") {
            make_windows_join_all_spaces(app);
            position_main_under_tray(app);
            panel.show();
            emit_shown(app);
            return;
        }
    }
    if let Some(w) = app.get_webview_window("main") {
        make_windows_join_all_spaces(app);
        position_main_under_tray(app);
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        emit_shown(app);
    }
}

/// Build the tray icon (menu-bar entry) with its menu and click behavior.
pub fn setup(app: &tauri::App) -> tauri::Result<()> {
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
        "../../icons/tray.png"
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
                        hide_main(app);
                    } else {
                        position_under_tray(&window, &rect);
                        show_main(app);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
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
