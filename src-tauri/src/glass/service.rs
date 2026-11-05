//! Keeps the glass picture current: built once at start and again whenever Windows reports a
//! wallpaper change (or the screen layout changes). Nothing is polled.

use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};

use base64::Engine;
use tauri::{AppHandle, Emitter, Manager};

use super::{brightness_grid, compose, output_size, GlassFrame, GLASS_CHANGED, GRID_COLS, GRID_ROWS};
use crate::{error::AppResult, shell::placement::Rect};

#[derive(Default)]
pub struct GlassState {
    frame: Mutex<Option<GlassFrame>>,
}

pub fn current(app: &AppHandle) -> Option<GlassFrame> {
    app.state::<GlassState>()
        .frame
        .lock()
        .expect("glass lock")
        .clone()
}

/// Rebuild the picture soon; bursts of change notices collapse into one rebuild.
pub fn refresh_soon(app: &AppHandle) {
    static PENDING: AtomicBool = AtomicBool::new(false);
    if PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        PENDING.store(false, Ordering::SeqCst);
        refresh(&app);
    });
}

fn refresh(app: &AppHandle) {
    let Some(screen) = crate::shell::dock_screen(app) else {
        return;
    };
    match build(screen) {
        Ok(frame) => {
            let state = app.state::<GlassState>();
            let mut current = state.frame.lock().expect("glass lock");
            if current.as_ref() == Some(&frame) {
                return;
            }
            *current = Some(frame.clone());
            drop(current);
            if let Err(error) = app.emit(GLASS_CHANGED, &frame) {
                tracing::warn!(%error, "failed to send the glass picture");
            }
        }
        // The dock still works without it: the page falls back to a plain tint.
        Err(error) => tracing::warn!(%error, "could not read the wallpaper for the glass"),
    }
}

#[cfg(windows)]
fn build(screen: Rect) -> AppResult<GlassFrame> {
    let wallpaper = super::win::read(screen)?;
    let picture = match &wallpaper.path {
        Some(path) => match image::open(path) {
            Ok(img) => Some(img.to_rgb8()),
            Err(error) => {
                tracing::warn!(%error, "could not open the wallpaper picture");
                None
            }
        },
        None => None,
    };
    let (width, height) = output_size(screen.width, screen.height);
    let source_scale = width as f64 / screen.width.max(1) as f64;
    let composed = compose(
        picture.as_ref(),
        wallpaper.fit,
        wallpaper.background,
        width,
        height,
        source_scale,
    );
    let brightness = brightness_grid(&composed, GRID_COLS, GRID_ROWS);
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut Cursor::new(&mut jpeg), 82)
        .encode_image(&composed)
        .map_err(|e| crate::error::AppError::Platform(format!("could not encode the glass picture: {e}")))?;
    Ok(GlassFrame {
        image: format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(jpeg)
        ),
        brightness,
        cols: GRID_COLS,
        rows: GRID_ROWS,
    })
}

#[cfg(not(windows))]
fn build(_screen: Rect) -> AppResult<GlassFrame> {
    Err(crate::error::AppError::Platform(
        "no wallpaper on this platform".into(),
    ))
}
