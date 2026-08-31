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

/// Apply the configured window geometry: fixed/resizable size and, in float
/// mode, the remembered position. Called at startup and when prefs change.
pub fn apply_window_geometry(app: &tauri::AppHandle) {
    use tauri::{LogicalSize, PhysicalPosition, Position, Size};

    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let state = app.state::<crate::app::state::AppState>();
    let (float, pos, fixed, (w, h)) = state.service.window_prefs();

    let _ = window.set_resizable(!fixed);
    let _ = window.set_size(Size::Logical(LogicalSize {
        width: w as f64,
        height: h as f64,
    }));
    if float {
        if let Some((x, y)) = pos {
            let _ = window.set_position(Position::Physical(PhysicalPosition { x, y }));
        }
    }
}

/// Position the window for showing: pinned mode anchors under the tray icon,
/// float mode restores the remembered position (or stays where it is).
fn position_main_under_tray(app: &tauri::AppHandle) {
    let state = app.state::<crate::app::state::AppState>();
    let (float, pos, _, (win_w, _)) = state.service.window_prefs();
    if float {
        if let (Some((x, y)), Some(window)) = (pos, app.get_webview_window("main")) {
            use tauri::{PhysicalPosition, Position};
            let _ = window.set_position(Position::Physical(PhysicalPosition { x, y }));
        }
        return;
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // No tray rect on GTK/SNI trays — the popover pins to the corner there.
    let rect = app
        .tray_by_id("main-tray")
        .and_then(|tray| tray.rect().ok().flatten());
    position_popover(&window, rect.as_ref(), win_w);
}

/// Ask the window manager for the keyboard, stamped with the current X server
/// time so the request reads as user-initiated rather than a background app
/// grabbing focus.
#[cfg(not(target_os = "macos"))]
fn present_with_server_time(gtk_window: &gtk::ApplicationWindow) {
    use gtk::prelude::*;

    let Some(gdk_window) = gtk_window.window() else {
        return;
    };
    let Ok(x11) = gdk_window.downcast::<gdkx11::X11Window>() else {
        return; // Wayland — nothing to time-stamp, the plain request is all we have.
    };
    let time = gdkx11::functions::x11_get_server_time(&x11);
    x11.set_user_time(time);
    gtk_window.present_with_time(time);
}

/// Take the keyboard every time the popover appears.
///
/// `show()` only queues the map — tao posts a request onto the main-loop
/// channel and returns — so a focus request issued right after it names a
/// window that is still unmapped, and a window manager drops those on the
/// floor. Measured on GNOME: asking straight after `show()` won the keyboard
/// in none of six tries, asking once the window was up won it in all five.
/// The `map` signal fires after `gdk_window_show()` has put the map on the
/// wire, so the request that follows is guaranteed to reach the window manager
/// behind it. Hooked once, for the window's lifetime.
#[cfg(not(target_os = "macos"))]
pub fn arm_focus_on_map(app: &tauri::AppHandle) {
    use gtk::prelude::*;

    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(gtk_window) = window.gtk_window() else {
        return;
    };
    gtk_window.connect_map(present_with_server_time);
}

/// Focus a popover that a show found already on screen — no map fires for it,
/// so `arm_focus_on_map` never gets a turn.
#[cfg(not(target_os = "macos"))]
fn focus_popover(window: &tauri::WebviewWindow) {
    let _ = window.set_focus();
    if let Ok(gtk_window) = window.gtk_window() {
        present_with_server_time(&gtk_window);
    }
}

/// The panel path takes key focus on its own; this is only reached when the
/// window never became one.
#[cfg(target_os = "macos")]
fn focus_popover(window: &tauri::WebviewWindow) {
    let _ = window.set_focus();
}

/// Hide the main window (panel-aware). The single hide path for every
/// trigger — hotkey toggle, tray click, auto-hide, frontend, close button.
/// Also the debounce point for persisting remembered window geometry.
pub fn hide_main(app: &tauri::AppHandle) {
    app.state::<crate::app::state::AppState>().service.persist();
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
            // Keep-above so the popover rises over open apps (Linux WMs may
            // drop the flag across hide/show cycles — re-assert every time).
            let _ = w.set_always_on_top(true);
            let _ = w.unminimize();
            let _ = w.show();
            focus_popover(&w);
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
        let _ = w.set_always_on_top(true);
        let _ = w.show();
        let _ = w.unminimize();
        focus_popover(&w);
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
                let _ = rect;
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    log::info!("tray clicked; window visible = {visible}");
                    if visible {
                        hide_main(app);
                    } else {
                        // show_main positions per the window mode (pinned/float).
                        show_main(app);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Place the popover for showing: under the tray icon when the platform
/// reports its geometry, pinned to the top-right corner when it does not.
/// The arithmetic lives in `domain::popover` (unit-tested); this only turns
/// Tauri types into it and back.
///
/// `width` is the configured window width in logical pixels. It comes from
/// the config rather than `outer_size()` on purpose: positioning runs right
/// after the startup resize, and GTK still reports the pre-resize size there
/// — placing the popover hundreds of pixels off the corner.
fn position_popover(window: &tauri::WebviewWindow, rect: Option<&tauri::Rect>, width: u32) {
    use crate::domain::popover::{popover_position, MonitorBounds, TrayAnchor};
    use tauri::{PhysicalPosition, Position};

    let sf = window.scale_factor().unwrap_or(1.0);
    let win_w = (width as f64 * sf).round() as u32;

    // Monitor bounds unknown -> centering is the only safe placement.
    let Ok(Some(monitor)) = window.primary_monitor() else {
        let _ = window.center();
        return;
    };

    // Tray rect -> physical pixels. `None` on GTK/SNI, where the tray
    // protocol carries no geometry at all (tray-icon returns a hard None).
    #[allow(unused_mut)]
    let mut anchor = rect.map(|r| TrayAnchor {
        x: match r.position {
            Position::Physical(p) => p.x as f64,
            Position::Logical(p) => p.x * sf,
        },
        width: match r.size {
            tauri::Size::Physical(s) => s.width as f64,
            tauri::Size::Logical(s) => s.width * sf,
        },
    });

    // No rect (GTK/SNI): use the icon position recorded in the config.
    if anchor.is_none() {
        if let Some(x) = window
            .app_handle()
            .state::<crate::app::state::AppState>()
            .service
            .tray_anchor_x()
        {
            // Width 0: the recorded x IS the icon's center, so the window
            // centers on it exactly as it does under a reported tray rect.
            anchor = Some(TrayAnchor {
                x: x as f64,
                width: 0.0,
            });
        }
    }

    // macOS: just below the menu bar (~24pt), as before. Elsewhere: below
    // whatever the desktop reserves at the top — the work area already
    // excludes GNOME's top bar, so no magic constant is needed.
    #[cfg(target_os = "macos")]
    let (bounds, top_margin) = (
        MonitorBounds {
            x: monitor.position().x,
            y: monitor.position().y,
            width: monitor.size().width,
        },
        26.0 * sf,
    );
    #[cfg(not(target_os = "macos"))]
    let (bounds, top_margin) = {
        let work = monitor.work_area();
        (
            MonitorBounds {
                x: work.position.x,
                y: work.position.y,
                width: work.size.width,
            },
            8.0,
        )
    };

    let (x, y) = popover_position(&bounds, win_w, anchor, top_margin);
    let _ = window.set_position(Position::Physical(PhysicalPosition { x, y }));
}
