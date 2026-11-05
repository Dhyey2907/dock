//! Keeps each dock's glass picture current: built once at start and again whenever Windows
//! reports a wallpaper change, the screen layout changes, or docks come and go. Each dock gets
//! the desktop of its own screen. Nothing is polled.

use std::{
    collections::HashMap,
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
    /// The picture each dock last got, by window label.
    frames: Mutex<HashMap<String, GlassFrame>>,
}

pub fn current(app: &AppHandle, label: &str) -> Option<GlassFrame> {
    app.state::<GlassState>()
        .frames
        .lock()
        .expect("glass lock")
        .get(label)
        .cloned()
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
    let docks = crate::shell::dock_screens(app);
    // Screens built in this pass, so two docks on one screen (never, today) share the work.
    let mut built: Vec<(Rect, GlassFrame)> = Vec::new();
    let state = app.state::<GlassState>();
    for (label, screen) in &docks {
        let frame = match built.iter().find(|(r, _)| r == screen) {
            Some((_, frame)) => frame.clone(),
            None => match build(*screen) {
                Ok(frame) => {
                    built.push((*screen, frame.clone()));
                    frame
                }
                // The dock still works without it: the page falls back to a plain tint.
                Err(error) => {
                    tracing::warn!(%error, label, "could not read the wallpaper for the glass");
                    continue;
                }
            },
        };
        let mut frames = state.frames.lock().expect("glass lock");
        if frames.get(label) == Some(&frame) {
            continue;
        }
        frames.insert(label.clone(), frame.clone());
        drop(frames);
        if let Err(error) = app.emit_to(label.as_str(), GLASS_CHANGED, &frame) {
            tracing::warn!(%error, "failed to send the glass picture");
        }
    }
    // Forget docks that are gone.
    state
        .frames
        .lock()
        .expect("glass lock")
        .retain(|label, _| docks.iter().any(|(l, _)| l == label));
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
