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
    let height = px(settings.size.height()).min(monitor.height as i64);
    let (width, margin) = match settings.style {
        DockStyle::Full => (monitor.width as i64, 0),
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
}
