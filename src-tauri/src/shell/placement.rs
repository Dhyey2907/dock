//! Where the dock window goes on a monitor. Pure math in physical pixels, so it is testable.

use crate::settings::{DockStyle, Edge, Settings};

/// Gap between a pill dock and the screen edge, in logical pixels.
pub const PILL_MARGIN: f64 = 8.0;
/// Narrowest pill, in logical pixels, so an empty dock is still a target.
pub const MIN_PILL_WIDTH: f64 = 160.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The dock's rectangle on a monitor whose full area is `monitor` at `scale` (1.5 = 150%).
/// `content_width` is the pill's content width in logical pixels, as the page measured it.
pub fn dock_rect(monitor: Rect, scale: f64, settings: &Settings, content_width: f64) -> Rect {
    let px = |logical: f64| (logical * scale).round() as i64;
    let height = px(settings.dock_height()).min(monitor.height as i64);
    let (width, margin) = match settings.style {
        DockStyle::Full => (monitor.width as i64, 0),
        // Two pills across the screen: the window spans it, inset like a pill.
        DockStyle::Split => {
            let margin = px(PILL_MARGIN);
            ((monitor.width as i64 - 2 * margin).max(1), margin)
        }
        DockStyle::Pill => {
            let margin = px(PILL_MARGIN);
            let max = (monitor.width as i64 - 2 * margin).max(1);
            (px(content_width.max(MIN_PILL_WIDTH)).min(max), margin)
        }
    };
    let x = monitor.x as i64 + (monitor.width as i64 - width) / 2;
    let y = match settings.edge {
        Edge::Bottom => monitor.y as i64 + monitor.height as i64 - height - margin,
        Edge::Top => monitor.y as i64 + margin,
    };
    Rect {
        x: x as i32,
        y: y as i32,
        width: width as u32,
        height: height as u32,
    }
}

/// The screen strip the dock claims so maximized windows stop short of it: the whole monitor
/// width, from the edge to the dock's far side (a pill's margin included).
pub fn reserved_rect(monitor: Rect, dock: Rect, edge: Edge) -> Rect {
    let (top, bottom) = match edge {
        Edge::Bottom => (dock.y, monitor.y + monitor.height as i32),
        Edge::Top => (monitor.y, dock.y + dock.height as i32),
    };
    Rect {
        x: monitor.x,
        y: top,
        width: monitor.width,
        height: (bottom - top).max(0) as u32,
    }
}

/// The part of a bottom `strip` above `free_bottom`, where a hidden taskbar's own strip begins.
/// Empty when the taskbar's strip already covers it.
pub fn above_taskbar(strip: Rect, free_bottom: i32) -> Rect {
    let bottom = free_bottom.min(strip.y + strip.height as i32);
    Rect {
        height: (bottom - strip.y).max(0) as u32,
        ..strip
    }
}

/// Height in physical pixels of the strip an auto-hidden dock leaves to catch the cursor.
pub const SLIVER: u32 = 3;

/// Where an auto-hidden dock waits: a thin full-width strip along its edge.
pub fn sliver_rect(monitor: Rect, edge: Edge) -> Rect {
    let y = match edge {
        Edge::Bottom => monitor.y + monitor.height as i32 - SLIVER as i32,
        Edge::Top => monitor.y,
    };
    Rect {
        x: monitor.x,
        y,
        width: monitor.width,
        height: SLIVER,
    }
}

/// The edge a dragged dock snaps to: whichever half of the screen its middle was dropped in.
pub fn edge_for_drop(monitor: Rect, window: Rect) -> Edge {
    let middle = window.y as i64 + window.height as i64 / 2;
    if middle < monitor.y as i64 + monitor.height as i64 / 2 {
        Edge::Top
    } else {
        Edge::Bottom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::DockSize;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        width: 2880,
        height: 1800,
    };

    #[test]
    fn full_width_bottom_spans_the_screen() {
        let settings = Settings {
            style: DockStyle::Full,
            ..Settings::default()
        };
        let rect = dock_rect(SCREEN, 1.5, &settings, 500.0);
        assert_eq!(
            rect,
            Rect {
                x: 0,
                y: 1800 - 84,
                width: 2880,
                height: 84
            }
        );
    }

    #[test]
    fn pill_is_centered_and_floats_above_the_edge() {
        let rect = dock_rect(SCREEN, 1.5, &Settings::default(), 600.0);
        assert_eq!(rect.width, 900);
        assert_eq!(rect.x, (2880 - 900) / 2);
        assert_eq!(rect.y, 1800 - 84 - 12);
    }

    #[test]
    fn pill_on_top_sits_below_the_top_edge() {
        let settings = Settings {
            edge: Edge::Top,
            size: DockSize::Small,
            ..Settings::default()
        };
        let rect = dock_rect(SCREEN, 1.0, &settings, 400.0);
        assert_eq!((rect.y, rect.height), (8, 48));
    }

    #[test]
    fn pill_width_is_clamped() {
        let tiny = dock_rect(SCREEN, 1.0, &Settings::default(), 10.0);
        assert_eq!(tiny.width, MIN_PILL_WIDTH as u32);
        let huge = dock_rect(SCREEN, 1.0, &Settings::default(), 99_999.0);
        assert_eq!(huge.width, 2880 - 16);
    }

    #[test]
    fn secondary_monitor_offsets_apply() {
        let monitor = Rect {
            x: -1920,
            y: 200,
            width: 1920,
            height: 1080,
        };
        let settings = Settings {
            style: DockStyle::Full,
            ..Settings::default()
        };
        let rect = dock_rect(monitor, 1.0, &settings, 0.0);
        assert_eq!((rect.x, rect.y), (-1920, 200 + 1080 - 56));
    }

    #[test]
    fn each_screen_gets_its_own_dock_strip_and_sliver() {
        // The user's layout: a 150% laptop screen and a 100% screen to its right.
        let laptop = Rect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1600,
        };
        let side = Rect {
            x: 2560,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let settings = Settings::default();
        let a = dock_rect(laptop, 1.5, &settings, 600.0);
        let b = dock_rect(side, 1.0, &settings, 600.0);
        assert_eq!((a.width, a.height, a.y), (900, 84, 1600 - 84 - 12));
        assert_eq!((b.width, b.height, b.y), (600, 56, 1080 - 56 - 8));
        assert_eq!(b.x, 2560 + (1920 - 600) / 2);
        let strip = reserved_rect(side, b, Edge::Bottom);
        assert_eq!((strip.x, strip.width, strip.y), (2560, 1920, 1080 - 64));
        assert_eq!(sliver_rect(side, Edge::Bottom).x, 2560);
    }

    #[test]
    fn claims_only_what_the_hidden_taskbar_leaves() {
        let strip = Rect {
            x: 0,
            y: 1520,
            width: 2560,
            height: 80,
        };
        // A taller dock claims the slice above the taskbar's strip.
        assert_eq!(above_taskbar(strip, 1528), Rect { height: 8, ..strip });
        // No taskbar strip (auto-hide was already on): the whole strip.
        assert_eq!(above_taskbar(strip, 1600), strip);
        // The taskbar's strip is at least as tall as the dock: nothing to claim.
        assert_eq!(above_taskbar(strip, 1520).height, 0);
        assert_eq!(above_taskbar(strip, 1500).height, 0);
    }

    #[test]
    fn reserved_strip_covers_the_pill_and_its_margin() {
        let dock = dock_rect(SCREEN, 1.5, &Settings::default(), 600.0);
        let strip = reserved_rect(SCREEN, dock, Edge::Bottom);
        assert_eq!(
            strip,
            Rect {
                x: 0,
                y: 1800 - 84 - 12,
                width: 2880,
                height: 84 + 12
            }
        );
    }

    #[test]
    fn reserved_strip_on_top_ends_below_the_dock() {
        let settings = Settings {
            edge: Edge::Top,
            ..Settings::default()
        };
        let dock = dock_rect(SCREEN, 1.0, &settings, 600.0);
        let strip = reserved_rect(SCREEN, dock, Edge::Top);
        assert_eq!((strip.y, strip.height), (0, 8 + 56));
    }

    #[test]
    fn sliver_hugs_the_edge() {
        assert_eq!(sliver_rect(SCREEN, Edge::Bottom).y, 1800 - SLIVER as i32);
        assert_eq!(sliver_rect(SCREEN, Edge::Top).y, 0);
        assert_eq!(sliver_rect(SCREEN, Edge::Top).width, 2880);
    }

    #[test]
    fn dropping_in_the_upper_half_snaps_to_the_top() {
        let at = |y| Rect {
            x: 100,
            y,
            width: 600,
            height: 84,
        };
        assert_eq!(edge_for_drop(SCREEN, at(300)), Edge::Top);
        assert_eq!(edge_for_drop(SCREEN, at(1200)), Edge::Bottom);
        // Exactly in the middle counts as the bottom, where it started.
        assert_eq!(edge_for_drop(SCREEN, at(900 - 42)), Edge::Bottom);
    }
}
