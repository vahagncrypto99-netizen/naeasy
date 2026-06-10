//! Tray icon (menu-bar entry) and main-window show/hide/position plumbing.

use tauri::Manager;

/// Toggle the main window from the global hotkey.
pub fn toggle_main(app: &tauri::AppHandle) {
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

/// Show, un-minimize and focus the main window.
pub fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
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
