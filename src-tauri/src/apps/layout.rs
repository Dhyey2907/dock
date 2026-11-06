//! Window geometry for the app features, kept pure: where a window snaps when an icon is
//! dropped at a screen edge, where it lands on another screen, how window previews are laid
//! out, and which windows move when a screen is unplugged or comes back. All physical pixels
//! unless a name says logical.

use std::collections::HashMap;

use serde::Serialize;
use ts_rs::TS;

use crate::shell::placement::Rect;

/// A connected screen: its device name, full rectangle and work area (without the strips the
/// docks and taskbar keep).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub name: String,
    pub rect: Rect,
    pub work: Rect,
    pub primary: bool,
}

fn contains(r: Rect, (x, y): (i32, i32)) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.width as i32 && y < r.y + r.height as i32
}

/// The screen a point is on, or the nearest one.
pub fn screen_at(screens: &[Screen], point: (i32, i32)) -> Option<&Screen> {
    screens.iter().find(|s| contains(s.rect, point)).or_else(|| {
        screens.iter().min_by_key(|s| {
            let cx = point.0.clamp(s.rect.x, s.rect.x + s.rect.width as i32);
            let cy = point.1.clamp(s.rect.y, s.rect.y + s.rect.height as i32);
            i64::from(cx - point.0).pow(2) + i64::from(cy - point.1).pow(2)
        })
    })
}

// ---------- Edge snapping ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Snap {
    Maximize,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Where an icon dropped at `point` snaps its window, if it was dropped near an edge of
/// `screen`: the top maximizes, the sides take halves and the corners quarters. The bottom
/// edge alone does nothing (the dock is usually there).
pub fn snap_zone(point: (i32, i32), screen: Rect) -> Option<Snap> {
    let band_x = (screen.width as i32 / 8).max(48);
    let band_y = (screen.height as i32 / 8).max(48);
    let left = point.0 < screen.x + band_x;
    let right = point.0 >= screen.x + screen.width as i32 - band_x;
    let top = point.1 < screen.y + band_y;
    let bottom = point.1 >= screen.y + screen.height as i32 - band_y;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(Snap::TopLeft),
        (_, true, true, _) => Some(Snap::TopRight),
        (true, _, _, true) => Some(Snap::BottomLeft),
        (_, true, _, true) => Some(Snap::BottomRight),
        (true, _, _, _) => Some(Snap::Left),
        (_, true, _, _) => Some(Snap::Right),
        (_, _, true, _) => Some(Snap::Maximize),
        _ => None,
    }
}

/// The rectangle a snap fills in a work area; `None` for maximizing.
pub fn snap_rect(snap: Snap, work: Rect) -> Option<Rect> {
    let half_w = work.width / 2;
    let half_h = work.height / 2;
    let right_x = work.x + half_w as i32;
    let lower_y = work.y + half_h as i32;
    let r = |x, y, width, height| Some(Rect { x, y, width, height });
    match snap {
        Snap::Maximize => None,
        Snap::Left => r(work.x, work.y, half_w, work.height),
        Snap::Right => r(right_x, work.y, work.width - half_w, work.height),
        Snap::TopLeft => r(work.x, work.y, half_w, half_h),
        Snap::TopRight => r(right_x, work.y, work.width - half_w, half_h),
        Snap::BottomLeft => r(work.x, lower_y, half_w, work.height - half_h),
        Snap::BottomRight => r(right_x, lower_y, work.width - half_w, work.height - half_h),
    }
}

// ---------- Moving between screens ----------

/// `window` moved from work area `from` to `to`, keeping its place relative to the work area
/// and its size (shrunk if the new screen is smaller), and kept fully on the new screen.
pub fn move_to_screen(window: Rect, from: Rect, to: Rect) -> Rect {
    let width = window.width.min(to.width);
    let height = window.height.min(to.height);
    let slack = |outer: u32, inner: u32| (outer.saturating_sub(inner)) as f64;
    let fraction = |offset: i32, outer: u32, inner: u32| {
        let room = slack(outer, inner);
        if room <= 0.0 {
            0.0
        } else {
            (f64::from(offset) / room).clamp(0.0, 1.0)
        }
    };
    let fx = fraction(window.x - from.x, from.width, window.width);
    let fy = fraction(window.y - from.y, from.height, window.height);
    Rect {
        x: to.x + (fx * slack(to.width, width)).round() as i32,
        y: to.y + (fy * slack(to.height, height)).round() as i32,
        width,
        height,
    }
}

/// The screen after `current` in a stable order (left to right, then top to bottom), for
/// "Move to other screen".
pub fn next_screen<'a>(screens: &'a [Screen], current: &str) -> Option<&'a Screen> {
    if screens.len() < 2 {
        return None;
    }
    let mut order: Vec<&Screen> = screens.iter().collect();
    order.sort_by_key(|s| (s.rect.x, s.rect.y));
    let at = order.iter().position(|s| s.name == current)?;
    Some(order[(at + 1) % order.len()])
}

// ---------- Unplugging and plugging back in ----------

/// Where a window was, as the dock last saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub hwnd: isize,
    /// Its screen's device name.
    pub monitor: String,
    /// Its rectangle when not maximized or minimized.
    pub rect: Rect,
    pub maximized: bool,
}

/// A window moved off a screen that went away, to put back when it returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Displaced {
    pub placement: Placement,
    /// The work area it was in.
    pub work: Rect,
}

/// What to do after the screens changed: windows that were on a screen that is gone move to
/// the main screen (and are remembered), and remembered windows whose screen is back go home.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Rearrange {
    /// Windows to move now: handle, new rectangle, maximize after moving.
    pub moves: Vec<(isize, Rect, bool)>,
    /// Newly displaced windows to remember.
    pub displaced: Vec<Displaced>,
    /// Remembered windows that went home (forget them).
    pub returned: Vec<isize>,
}

/// Plan the moves after the screens changed from `before` to `now`. `placements` is where
/// windows were before the change; `remembered` the windows moved off screens earlier; `alive`
/// says whether a window still exists.
pub fn rearrange(
    before: &[Screen],
    now: &[Screen],
    placements: &[Placement],
    remembered: &HashMap<isize, Displaced>,
    alive: impl Fn(isize) -> bool,
) -> Rearrange {
    let mut plan = Rearrange::default();
    let connected = |name: &str| now.iter().any(|s| s.name == name);
    let Some(main) = now.iter().find(|s| s.primary).or(now.first()) else {
        return plan;
    };
    for p in placements {
        if connected(&p.monitor) || remembered.contains_key(&p.hwnd) || !alive(p.hwnd) {
            continue;
        }
        let Some(gone) = before.iter().find(|s| s.name == p.monitor) else {
            continue;
        };
        plan.moves
            .push((p.hwnd, move_to_screen(p.rect, gone.work, main.work), p.maximized));
        plan.displaced.push(Displaced {
            placement: p.clone(),
            work: gone.work,
        });
    }
    let mut back: Vec<&Displaced> = remembered.values().collect();
    back.sort_by_key(|d| d.placement.hwnd);
    for d in back {
        if !alive(d.placement.hwnd) {
            plan.returned.push(d.placement.hwnd);
            continue;
        }
        let Some(home) = now.iter().find(|s| s.name == d.placement.monitor) else {
            continue;
        };
        // The same screen may come back with another resolution or place.
        let rect = move_to_screen(d.placement.rect, d.work, home.work);
        plan.moves.push((d.placement.hwnd, rect, d.placement.maximized));
        plan.returned.push(d.placement.hwnd);
    }
    plan
}

// ---------- Window previews ----------

/// One window's card in the preview popup, in logical pixels from the popup's top-left.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PreviewCard {
    #[ts(type = "number")]
    pub hwnd: isize,
    pub title: String,
    pub active: bool,
    pub minimized: bool,
    pub hung: bool,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Where the live picture goes (inside the card, below the title).
    pub thumb_x: f64,
    pub thumb_y: f64,
    pub thumb_width: f64,
    pub thumb_height: f64,
}

pub const PREVIEW_PADDING: f64 = 10.0;
pub const PREVIEW_GAP: f64 = 8.0;
pub const PREVIEW_TITLE: f64 = 30.0;
pub const PREVIEW_INSET: f64 = 6.0;
/// Most cards in one popup.
pub const PREVIEW_MAX: usize = 6;

/// Cards in a row, each as tall as `thumb_height` plus a title, as wide as its window's shape
/// allows (between 120 and 300), shrunk together when the row is wider than `max_width`.
/// `windows` gives each window's size; a zero size (minimized) gets a square-ish card.
/// Returns the popup's logical size and the cards.
pub fn preview_layout(
    windows: &[(isize, String, bool, bool, bool, u32, u32)],
    thumb_height: f64,
    max_width: f64,
) -> ((f64, f64), Vec<PreviewCard>) {
    let windows = &windows[..windows.len().min(PREVIEW_MAX)];
    let widths: Vec<f64> = windows
        .iter()
        .map(|w| {
            let (sw, sh) = (f64::from(w.5), f64::from(w.6));
            let thumb = if sw > 0.0 && sh > 0.0 {
                thumb_height * sw / sh
            } else {
                thumb_height * 1.4
            };
            thumb.clamp(120.0, 300.0) + 2.0 * PREVIEW_INSET
        })
        .collect();
    let gaps = PREVIEW_GAP * windows.len().saturating_sub(1) as f64;
    let natural: f64 = widths.iter().sum::<f64>() + gaps + 2.0 * PREVIEW_PADDING;
    let scale = if natural > max_width && natural > 0.0 {
        ((max_width - gaps - 2.0 * PREVIEW_PADDING) / (natural - gaps - 2.0 * PREVIEW_PADDING)).max(0.3)
    } else {
        1.0
    };
    let thumb_h = (thumb_height * scale).round();
    let card_h = thumb_h + PREVIEW_TITLE + PREVIEW_INSET;
    let mut x = PREVIEW_PADDING;
    let mut cards = Vec::new();
    for (w, width) in windows.iter().zip(widths) {
        let card_w = (width * scale).round();
        let inner_w = card_w - 2.0 * PREVIEW_INSET;
        let (sw, sh) = (f64::from(w.5), f64::from(w.6));
        // Fit the window's shape inside the thumbnail area, centred.
        let (tw, th) = if sw > 0.0 && sh > 0.0 {
            let fit = (inner_w / sw).min(thumb_h / sh);
            ((sw * fit).round(), (sh * fit).round())
        } else {
            (inner_w, thumb_h)
        };
        cards.push(PreviewCard {
            hwnd: w.0,
            title: w.1.clone(),
            active: w.2,
            minimized: w.3,
            hung: w.4,
            x,
            y: PREVIEW_PADDING,
            width: card_w,
            height: card_h,
            thumb_x: x + PREVIEW_INSET + ((inner_w - tw) / 2.0).round(),
            thumb_y: PREVIEW_PADDING + PREVIEW_TITLE + ((thumb_h - th) / 2.0).round(),
            thumb_width: tw,
            thumb_height: th,
        });
        x += card_w + PREVIEW_GAP;
    }
    let width = (x - PREVIEW_GAP + PREVIEW_PADDING).max(2.0 * PREVIEW_PADDING);
    ((width, card_h + 2.0 * PREVIEW_PADDING), cards)
}

/// A rectangle in an app button's logical pixels, relative to its dock window, made a screen
/// rectangle (for the minimize animation).
pub fn to_screen(x: f64, y: f64, width: f64, height: f64, origin: (i32, i32), scale: f64) -> Rect {
    Rect {
        x: origin.0 + (x * scale).round() as i32,
        y: origin.1 + (y * scale).round() as i32,
        width: (width * scale).round().max(1.0) as u32,
        height: (height * scale).round().max(1.0) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        width: 2000,
        height: 1200,
    };
    const WORK: Rect = Rect {
        x: 0,
        y: 0,
        width: 2000,
        height: 1100,
    };

    fn screen(name: &str, x: i32, primary: bool) -> Screen {
        let rect = Rect { x, ..SCREEN };
        Screen {
            name: name.into(),
            rect,
            work: Rect { x, ..WORK },
            primary,
        }
    }

    #[test]
    fn edges_and_corners_pick_snaps() {
        assert_eq!(snap_zone((10, 600), SCREEN), Some(Snap::Left));
        assert_eq!(snap_zone((1990, 600), SCREEN), Some(Snap::Right));
        assert_eq!(snap_zone((1000, 5), SCREEN), Some(Snap::Maximize));
        assert_eq!(snap_zone((5, 5), SCREEN), Some(Snap::TopLeft));
        assert_eq!(snap_zone((1995, 5), SCREEN), Some(Snap::TopRight));
        assert_eq!(snap_zone((5, 1195), SCREEN), Some(Snap::BottomLeft));
        assert_eq!(snap_zone((1995, 1195), SCREEN), Some(Snap::BottomRight));
        assert_eq!(snap_zone((1000, 1195), SCREEN), None);
        assert_eq!(snap_zone((1000, 600), SCREEN), None);
        // On a screen to the right of the main one.
        let second = Rect { x: 2000, ..SCREEN };
        assert_eq!(snap_zone((2010, 600), second), Some(Snap::Left));
    }

    #[test]
    fn snaps_fill_halves_and_quarters_of_the_work_area() {
        let odd = Rect {
            x: 100,
            y: 0,
            width: 1001,
            height: 801,
        };
        assert_eq!(snap_rect(Snap::Maximize, odd), None);
        let left = snap_rect(Snap::Left, odd).unwrap();
        let right = snap_rect(Snap::Right, odd).unwrap();
        assert_eq!(left.width + right.width, 1001);
        assert_eq!(right.x, left.x + left.width as i32);
        let br = snap_rect(Snap::BottomRight, odd).unwrap();
        assert_eq!((br.x, br.y, br.width, br.height), (600, 400, 501, 401));
    }

    #[test]
    fn moving_to_another_screen_keeps_the_relative_place_and_fits() {
        let from = Rect {
            x: 0,
            y: 0,
            width: 2000,
            height: 1000,
        };
        let to = Rect {
            x: 2000,
            y: 0,
            width: 1000,
            height: 500,
        };
        // Centred stays centred.
        let window = Rect {
            x: 750,
            y: 250,
            width: 500,
            height: 500,
        };
        assert_eq!(
            move_to_screen(window, from, to),
            Rect {
                x: 2250,
                y: 0,
                width: 500,
                height: 500
            }
        );
        // Too big for the new screen: shrunk to fit.
        let big = Rect {
            x: 0,
            y: 0,
            width: 1800,
            height: 900,
        };
        let moved = move_to_screen(big, from, to);
        assert_eq!((moved.x, moved.width, moved.height), (2000, 1000, 500));
    }

    #[test]
    fn next_screen_cycles_left_to_right() {
        let screens = [screen("B", 2000, false), screen("A", 0, true)];
        assert_eq!(next_screen(&screens, "A").unwrap().name, "B");
        assert_eq!(next_screen(&screens, "B").unwrap().name, "A");
        assert!(next_screen(&screens[..1], "B").is_none());
        assert!(next_screen(&screens, "C").is_none());
    }

    #[test]
    fn unplugging_moves_windows_home_and_back() {
        let before = [screen("A", 0, true), screen("B", 2000, false)];
        let after = [screen("A", 0, true)];
        let on_b = Placement {
            hwnd: 7,
            monitor: "B".into(),
            rect: Rect {
                x: 2100,
                y: 100,
                width: 800,
                height: 600,
            },
            maximized: true,
        };
        let on_a = Placement {
            hwnd: 8,
            monitor: "A".into(),
            ..on_b.clone()
        };
        let closed = Placement {
            hwnd: 9,
            ..on_b.clone()
        };
        let none = HashMap::new();
        let plan = rearrange(&before, &after, &[on_b.clone(), on_a, closed], &none, |h| h != 9);
        assert_eq!(plan.moves.len(), 1);
        let (hwnd, rect, maximize) = plan.moves[0];
        assert_eq!((hwnd, rect.x, rect.y, maximize), (7, 100, 100, true));
        assert_eq!(plan.displaced[0].placement, on_b);
        // B comes back: the window goes home.
        let remembered: HashMap<isize, Displaced> = plan
            .displaced
            .into_iter()
            .map(|d| (d.placement.hwnd, d))
            .collect();
        let back = rearrange(&after, &before, &[], &remembered, |_| true);
        assert_eq!(back.moves, vec![(7, on_b.rect, true)]);
        assert_eq!(back.returned, vec![7]);
        // A remembered window that closed meanwhile is just forgotten.
        let gone = rearrange(&after, &before, &[], &remembered, |_| false);
        assert!(gone.moves.is_empty());
        assert_eq!(gone.returned, vec![7]);
        // While B is still away, nothing moves twice.
        let still = rearrange(&after, &after, &[on_b], &remembered, |_| true);
        assert!(still.moves.is_empty() && still.returned.is_empty());
    }

    #[test]
    fn screen_at_finds_the_nearest_screen() {
        let screens = [screen("A", 0, true), screen("B", 2000, false)];
        assert_eq!(screen_at(&screens, (2500, 10)).unwrap().name, "B");
        assert_eq!(screen_at(&screens, (-50, 10)).unwrap().name, "A");
    }

    #[test]
    fn previews_fit_window_shapes_in_a_row() {
        let windows = vec![
            (1, "Wide".to_string(), true, false, false, 1920, 1080),
            (2, "Minimized".to_string(), false, true, false, 0, 0),
        ];
        let ((w, h), cards) = preview_layout(&windows, 140.0, 2000.0);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].x, PREVIEW_PADDING);
        assert_eq!(cards[1].x, cards[0].x + cards[0].width + PREVIEW_GAP);
        assert_eq!(w, cards[1].x + cards[1].width + PREVIEW_PADDING);
        assert_eq!(h, cards[0].height + 2.0 * PREVIEW_PADDING);
        // 16:9 at 140 tall is ~249 wide; the picture keeps that shape inside the card.
        assert!((cards[0].thumb_width / cards[0].thumb_height - 16.0 / 9.0).abs() < 0.02);
        assert!(
            cards[0].thumb_x >= cards[0].x
                && cards[0].thumb_x + cards[0].thumb_width <= cards[0].x + cards[0].width
        );
    }

    #[test]
    fn previews_shrink_to_fit_the_screen_and_cap_the_count() {
        let windows: Vec<_> = (0..9)
            .map(|i| (i, String::new(), false, false, false, 1600, 900))
            .collect();
        let ((w, _), cards) = preview_layout(&windows, 140.0, 900.0);
        assert_eq!(cards.len(), PREVIEW_MAX);
        assert!(w <= 901.0, "{w}");
    }

    #[test]
    fn button_rects_become_screen_rects() {
        let r = to_screen(10.0, 4.0, 40.0, 40.0, (700, 1500), 1.5);
        assert_eq!(
            r,
            Rect {
                x: 715,
                y: 1506,
                width: 60,
                height: 60
            }
        );
    }
}
