//! The picture behind the glass: what the desktop shows on the dock's screen — the wallpaper
//! laid out the way Windows lays it out (fill, fit, stretch, center, tile, span) over its
//! background colour. The page draws the slice behind the dock, then bends and frosts it.
//!
//! This file is the pure part (layout, brightness); `win.rs` reads the wallpaper settings and
//! `service.rs` keeps the picture current.

pub mod service;
#[cfg(windows)]
mod win;

use image::{imageops, imageops::FilterType, Rgb, RgbImage};
use serde::Serialize;
use ts_rs::TS;

/// Event carrying a new `GlassFrame` whenever the wallpaper changes.
pub const GLASS_CHANGED: &str = "glass-changed";
/// Widest picture sent to the page; the glass is frosted, so more detail is never seen.
pub const MAX_WIDTH: u32 = 1920;
/// Brightness is measured on a coarse grid of this many columns and rows.
pub const GRID_COLS: u32 = 48;
pub const GRID_ROWS: u32 = 30;

/// How Windows lays the wallpaper out on a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Scaled to cover the screen, cropped.
    Fill,
    /// Scaled to fit inside the screen, background colour around it.
    Contain,
    /// Stretched to the screen's shape.
    Stretch,
    /// Actual size, centred.
    Center,
    /// Actual size, repeated from the top-left.
    Tile,
    /// One picture across all screens; on a single screen it looks like Fill.
    Span,
}

/// The desktop picture for the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GlassFrame {
    /// JPEG data URL of the whole screen's desktop, at most `MAX_WIDTH` wide.
    pub image: String,
    /// Brightness (0–1) of each grid cell, row by row (`GRID_COLS` × `GRID_ROWS`).
    pub brightness: Vec<f32>,
    pub cols: u32,
    pub rows: u32,
}

/// Lay `wallpaper` out on a `width`×`height` screen the way Windows would. `source_scale` maps
/// the wallpaper's own pixels to the screen's (Center and Tile keep actual size, so when the
/// output is smaller than the real screen they shrink by the same ratio).
pub fn compose(
    wallpaper: Option<&RgbImage>,
    fit: Fit,
    background: [u8; 3],
    width: u32,
    height: u32,
    source_scale: f64,
) -> RgbImage {
    let mut screen = RgbImage::from_pixel(width.max(1), height.max(1), Rgb(background));
    let Some(wallpaper) = wallpaper.filter(|w| w.width() > 0 && w.height() > 0) else {
        return screen;
    };
    let (ww, wh) = (wallpaper.width() as f64, wallpaper.height() as f64);
    let (sw, sh) = (width as f64, height as f64);
    let resized = |w: f64, h: f64| {
        imageops::resize(
            wallpaper,
            (w.round() as u32).max(1),
            (h.round() as u32).max(1),
            FilterType::Triangle,
        )
    };
    let centered = |img: &RgbImage| {
        (
            (width as i64 - img.width() as i64) / 2,
            (height as i64 - img.height() as i64) / 2,
        )
    };
    match fit {
        Fit::Fill | Fit::Span | Fit::Contain => {
            let cover = fit != Fit::Contain;
            let ratio = if cover {
                (sw / ww).max(sh / wh)
            } else {
                (sw / ww).min(sh / wh)
            };
            let img = resized(ww * ratio, wh * ratio);
            let (x, y) = centered(&img);
            imageops::overlay(&mut screen, &img, x, y);
        }
        Fit::Stretch => {
            let img = resized(sw, sh);
            imageops::overlay(&mut screen, &img, 0, 0);
        }
        Fit::Center => {
            let img = resized(ww * source_scale, wh * source_scale);
            let (x, y) = centered(&img);
            imageops::overlay(&mut screen, &img, x, y);
        }
        Fit::Tile => {
            let img = resized(ww * source_scale, wh * source_scale);
            let (tw, th) = (img.width() as i64, img.height() as i64);
            for y in (0..height as i64).step_by(th as usize) {
                for x in (0..width as i64).step_by(tw as usize) {
                    imageops::overlay(&mut screen, &img, x, y);
                }
            }
        }
    }
    screen
}

/// Relative luminance (0–1) of an sRGB pixel, close enough for picking light or dark text.
fn luminance(p: &Rgb<u8>) -> f32 {
    (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0
}

/// Average brightness of each cell of a `cols`×`rows` grid laid over `img`, row by row.
pub fn brightness_grid(img: &RgbImage, cols: u32, rows: u32) -> Vec<f32> {
    let small = imageops::resize(img, cols, rows, FilterType::Triangle);
    small.pixels().map(luminance).collect()
}

/// The output size for a `width`×`height` screen: its shape, at most `MAX_WIDTH` wide.
pub fn output_size(width: u32, height: u32) -> (u32, u32) {
    if width <= MAX_WIDTH || width == 0 {
        return (width.max(1), height.max(1));
    }
    let h = (height as u64 * MAX_WIDTH as u64 / width as u64) as u32;
    (MAX_WIDTH, h.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: [u8; 3] = [10, 20, 30];

    /// A wallpaper whose left half is red and right half is blue.
    fn halves(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, _| {
            if x < w / 2 {
                Rgb([255, 0, 0])
            } else {
                Rgb([0, 0, 255])
            }
        })
    }

    #[test]
    fn no_wallpaper_is_the_background_colour() {
        let screen = compose(None, Fit::Fill, BG, 8, 4, 1.0);
        assert!(screen.pixels().all(|p| p.0 == BG));
    }

    #[test]
    fn fill_covers_and_crops() {
        // A square on a wide screen: scaled to the width, top and bottom cropped, no background.
        let screen = compose(Some(&halves(10, 10)), Fit::Fill, BG, 40, 20, 1.0);
        assert!(screen.pixels().all(|p| p.0 != BG));
        assert_eq!(screen.get_pixel(2, 10).0, [255, 0, 0]);
        assert_eq!(screen.get_pixel(37, 10).0, [0, 0, 255]);
    }

    #[test]
    fn fit_letterboxes_with_the_background() {
        let screen = compose(Some(&halves(10, 10)), Fit::Contain, BG, 40, 20, 1.0);
        assert_eq!(screen.get_pixel(1, 10).0, BG);
        assert_eq!(screen.get_pixel(38, 10).0, BG);
        assert_eq!(screen.get_pixel(12, 10).0, [255, 0, 0]);
    }

    #[test]
    fn center_keeps_actual_size() {
        let screen = compose(Some(&halves(4, 4)), Fit::Center, BG, 20, 20, 1.0);
        assert_eq!(screen.get_pixel(0, 0).0, BG);
        assert_eq!(screen.get_pixel(8, 9).0, [255, 0, 0]);
        assert_eq!(screen.get_pixel(11, 9).0, [0, 0, 255]);
    }

    #[test]
    fn tile_repeats_from_the_corner() {
        let screen = compose(Some(&halves(4, 4)), Fit::Tile, BG, 12, 8, 1.0);
        for x in [0, 4, 8] {
            assert_eq!(screen.get_pixel(x, 6).0, [255, 0, 0]);
            assert_eq!(screen.get_pixel(x + 3, 6).0, [0, 0, 255]);
        }
    }

    #[test]
    fn stretch_fills_the_screen_shape() {
        let screen = compose(Some(&halves(10, 10)), Fit::Stretch, BG, 40, 10, 1.0);
        assert_eq!(screen.get_pixel(5, 5).0, [255, 0, 0]);
        assert_eq!(screen.get_pixel(35, 5).0, [0, 0, 255]);
    }

    #[test]
    fn brightness_grid_reads_light_and_dark_regions() {
        let img = RgbImage::from_fn(20, 10, |_, y| {
            if y < 5 {
                Rgb([255, 255, 255])
            } else {
                Rgb([0, 0, 0])
            }
        });
        let grid = brightness_grid(&img, 2, 2);
        assert!(grid[0] > 0.75 && grid[1] > 0.75, "top row is light: {grid:?}");
        assert!(grid[2] < 0.25 && grid[3] < 0.25, "bottom row is dark: {grid:?}");
    }

    #[test]
    fn output_keeps_the_shape_and_caps_the_width() {
        assert_eq!(output_size(2560, 1600), (1920, 1200));
        assert_eq!(output_size(1280, 720), (1280, 720));
    }
}
