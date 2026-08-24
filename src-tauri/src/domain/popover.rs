//! Where the popover window goes when it is shown.
//!
//! Pure geometry: the caller supplies monitor bounds, the window width and —
//! when the platform exposes it — the tray icon's horizontal position. macOS
//! reports a tray rect and the popover hangs under the icon; the GTK/SNI tray
//! reports none, so the popover falls back to the corner the indicators live
//! in. Everything is in physical pixels.

/// Gap kept between the popover and the screen edges.
const EDGE_MARGIN: f64 = 8.0;

/// Monitor bounds in physical pixels.
pub struct MonitorBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
}

/// Horizontal placement of the tray icon, in physical pixels.
pub struct TrayAnchor {
    pub x: f64,
    pub width: f64,
}

/// Top-left corner for the popover, clamped so it always stays on screen.
///
/// `anchor` present -> centered under the tray icon.
/// `anchor` absent  -> pinned to the top-right corner (GTK/SNI trays never
/// report their geometry, so there is nothing to center under).
pub fn popover_position(
    monitor: &MonitorBounds,
    window_width: u32,
    anchor: Option<TrayAnchor>,
    top_margin: f64,
) -> (i32, i32) {
    let win_w = window_width as f64;
    let mon_x = monitor.x as f64;
    let mon_w = monitor.width as f64;

    let mut x = match anchor {
        Some(a) => a.x + a.width / 2.0 - win_w / 2.0,
        None => mon_x + mon_w - win_w - EDGE_MARGIN,
    };

    // Clamp into the monitor. The left margin wins on screens too narrow to
    // hold the window, so the popover is never pushed off to the left.
    let min_x = mon_x + EDGE_MARGIN;
    let max_x = mon_x + mon_w - win_w - EDGE_MARGIN;
    if x < min_x {
        x = min_x;
    }
    if max_x > min_x && x > max_x {
        x = max_x;
    }

    (x as i32, (monitor.y as f64 + top_margin) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor() -> MonitorBounds {
        MonitorBounds { x: 0, y: 0, width: 1920 }
    }

    #[test]
    fn centers_under_the_tray_icon() {
        // Icon spanning 1000..1024 -> its center is 1012; a 380-wide window
        // starts 190 to the left of that.
        let pos = popover_position(
            &monitor(),
            380,
            Some(TrayAnchor { x: 1000.0, width: 24.0 }),
            26.0,
        );
        assert_eq!(pos, (822, 26));
    }

    #[test]
    fn pins_to_the_top_right_corner_without_an_anchor() {
        // 1920 - 380 - 8 = 1532
        let pos = popover_position(&monitor(), 380, None, 32.0);
        assert_eq!(pos, (1532, 32));
    }

    #[test]
    fn keeps_the_window_on_screen_when_the_icon_sits_at_the_left_edge() {
        let pos = popover_position(
            &monitor(),
            380,
            Some(TrayAnchor { x: 0.0, width: 24.0 }),
            26.0,
        );
        assert_eq!(pos.0, 8, "clamped to the left margin");
    }

    #[test]
    fn keeps_the_window_on_screen_when_the_icon_sits_at_the_right_edge() {
        let pos = popover_position(
            &monitor(),
            380,
            Some(TrayAnchor { x: 1910.0, width: 24.0 }),
            26.0,
        );
        assert_eq!(pos.0, 1532, "clamped to the right margin");
    }

    #[test]
    fn respects_a_monitor_that_does_not_start_at_zero() {
        // Secondary monitor to the right of the primary one.
        let monitor = MonitorBounds { x: 1920, y: -200, width: 1280 };
        let pos = popover_position(&monitor, 380, None, 26.0);
        assert_eq!(pos, (1920 + 1280 - 380 - 8, -200 + 26));
    }

    #[test]
    fn never_overflows_a_monitor_narrower_than_the_window() {
        let monitor = MonitorBounds { x: 0, y: 0, width: 300 };
        let pos = popover_position(&monitor, 380, None, 26.0);
        assert_eq!(pos.0, 8, "left margin wins when the window cannot fit");
    }
}
