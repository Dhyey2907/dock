//! The latest screenshot on the dock: the Screenshots folder (where Win+PrtScn and the Snipping
//! Tool save) is watched, and a new picture shows on the main dock as a small thumbnail for two
//! minutes. It can be dragged out as a file, or clicked to open.

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};

use base64::Engine;
use chrono::{DateTime, Utc};
use notify::{EventKind, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::{
    error::{AppError, AppResult},
    settings::SettingsStore,
};

/// Event carrying the screenshot to show, or null when there's none any more.
pub const LATEST_SCREENSHOT: &str = "latest-screenshot";
/// How long a new screenshot stays on the dock.
pub const SHOW_FOR: Duration = Duration::from_secs(120);
/// Thumbnail height in pixels (drawn at about 36 logical pixels, sharp up to 200% scaling).
const THUMB_HEIGHT: u32 = 72;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LatestScreenshot {
    pub path: String,
    /// A small PNG, as a data URL.
    pub thumb: String,
    #[ts(type = "string")]
    pub taken: DateTime<Utc>,
    /// When it leaves the dock.
    #[ts(type = "string")]
    pub until: DateTime<Utc>,
}

#[derive(Default)]
pub struct ScreenshotState {
    current: Mutex<Option<LatestScreenshot>>,
    /// Bumped for each new screenshot, so an older one's timer doesn't clear a newer one.
    generation: AtomicU64,
    /// Kept alive while the dock runs.
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
}

/// Whether `path` looks like a picture a screenshot tool saved.
pub fn is_picture(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp")
}

/// The Screenshots folder (it moves if the user moved it).
#[cfg(windows)]
fn folder() -> Option<PathBuf> {
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_Screenshots, SHGetKnownFolderPath, KF_FLAG_DEFAULT},
    };
    // SAFETY: the returned string is ours to free, and is freed after it is copied.
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Screenshots, KF_FLAG_DEFAULT, None).ok()?;
        let text = path.to_string().ok();
        CoTaskMemFree(Some(path.0 as *const std::ffi::c_void));
        text.map(PathBuf::from)
    }
}

#[cfg(not(windows))]
fn folder() -> Option<PathBuf> {
    None
}

/// A small PNG of the picture at `path`, as a data URL.
pub fn thumbnail(path: &Path) -> AppResult<String> {
    let picture = image::open(path).map_err(|e| AppError::Platform(e.to_string()))?;
    let small = picture.thumbnail(THUMB_HEIGHT * 3, THUMB_HEIGHT);
    let mut png = Vec::new();
    small
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| AppError::Platform(e.to_string()))?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

pub fn current(app: &AppHandle) -> Option<LatestScreenshot> {
    let shot = app
        .state::<ScreenshotState>()
        .current
        .lock()
        .expect("screenshot lock")
        .clone();
    shot.filter(|s| s.until > Utc::now())
}

fn publish(app: &AppHandle, shot: Option<LatestScreenshot>) {
    *app.state::<ScreenshotState>()
        .current
        .lock()
        .expect("screenshot lock") = shot.clone();
    if let Err(error) = app.emit_to(crate::shell::DOCK_LABEL, LATEST_SCREENSHOT, shot) {
        tracing::warn!(%error, "failed to send the screenshot");
    }
}

/// Take the screenshot off the dock now.
pub fn dismiss(app: &AppHandle) {
    app.state::<ScreenshotState>()
        .generation
        .fetch_add(1, Ordering::SeqCst);
    publish(app, None);
}

fn arrived(app: &AppHandle, path: PathBuf) {
    if !app.state::<SettingsStore>().get().latest_screenshot {
        return;
    }
    let state = app.state::<ScreenshotState>();
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        // The tool may still be writing; a moment later the file is whole.
        std::thread::sleep(Duration::from_millis(700));
        let state = app.state::<ScreenshotState>();
        if state.generation.load(Ordering::SeqCst) != generation {
            return;
        }
        let thumb = match thumbnail(&path) {
            Ok(thumb) => thumb,
            Err(error) => {
                tracing::debug!(%error, "screenshot not readable yet");
                return;
            }
        };
        let now = Utc::now();
        publish(
            &app,
            Some(LatestScreenshot {
                path: path.to_string_lossy().into_owned(),
                thumb,
                taken: now,
                until: now + chrono::Duration::from_std(SHOW_FOR).unwrap_or_default(),
            }),
        );
        std::thread::sleep(SHOW_FOR);
        if state.generation.load(Ordering::SeqCst) == generation {
            publish(&app, None);
        }
    });
}

pub fn init(app: &AppHandle) {
    let Some(dir) = folder() else {
        tracing::info!("no Screenshots folder; latest screenshot off");
        return;
    };
    let handle = app.clone();
    let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
            return;
        }
        if let Some(path) = event.paths.into_iter().rev().find(|p| is_picture(p)) {
            arrived(&handle, path);
        }
    });
    let mut watcher = match watcher {
        Ok(watcher) => watcher,
        Err(error) => {
            tracing::warn!(%error, "can't watch screenshots");
            return;
        }
    };
    if let Err(error) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
        tracing::info!(%error, "can't watch the Screenshots folder (it may not exist yet)");
        return;
    }
    *app.state::<ScreenshotState>()
        .watcher
        .lock()
        .expect("screenshot lock") = Some(watcher);
}

/// The screenshot on show, checked against what the dock holds.
fn showing(app: &AppHandle, path: &str) -> AppResult<String> {
    current(app)
        .map(|s| s.path)
        .filter(|p| p == path)
        .ok_or_else(|| AppError::InvalidInput("that screenshot isn't on the dock".into()))
}

pub fn open(app: &AppHandle, path: &str) -> AppResult<()> {
    let path = showing(app, path)?;
    #[cfg(windows)]
    crate::apps::win::launch(&crate::apps::Launch::Program(path));
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

/// Start dragging the screenshot out as a file, while the mouse button is still down. Runs the
/// drag on the window thread, which Windows' drag and drop needs.
pub fn drag(app: &AppHandle, window: &tauri::WebviewWindow, path: &str) -> AppResult<()> {
    let path = showing(app, path)?;
    #[cfg(windows)]
    {
        let hwnd = window.hwnd()?.0 as isize;
        app.run_on_main_thread(move || win::drag_file(hwnd, &path))?;
    }
    #[cfg(not(windows))]
    let _ = (window, path);
    Ok(())
}

#[cfg(windows)]
mod win {
    use windows::{
        core::HSTRING,
        Win32::{
            Foundation::HWND,
            System::{Com::IDataObject, Ole::DROPEFFECT_COPY},
            UI::Shell::{BHID_DataObject, IShellItem, SHCreateItemFromParsingName, SHDoDragDrop},
        },
    };

    pub fn drag_file(hwnd: isize, path: &str) {
        // SAFETY: COM calls on objects owned here, on the window thread (OLE is set up there by
        // the window library); the drag loop returns when the button is released.
        unsafe {
            let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(path), None) else {
                return;
            };
            let Ok(data) = item.BindToHandler::<_, IDataObject>(None, &BHID_DataObject) else {
                return;
            };
            if let Err(error) = SHDoDragDrop(
                Some(HWND(hwnd as *mut std::ffi::c_void)),
                &data,
                None,
                DROPEFFECT_COPY,
            ) {
                tracing::debug!(%error, "screenshot drag ended");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pictures_count() {
        assert!(is_picture(Path::new(
            r"C:\Users\a\Pictures\Screenshots\Screenshot 1.png"
        )));
        assert!(is_picture(Path::new("x.JPG")));
        assert!(!is_picture(Path::new("desktop.ini")));
        assert!(!is_picture(Path::new("Screenshot.png.tmp")));
    }

    #[test]
    fn thumbnails_are_small_pngs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.png");
        image::RgbaImage::from_pixel(1920, 1080, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .unwrap();
        let thumb = thumbnail(&path).unwrap();
        assert!(thumb.starts_with("data:image/png;base64,"));
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(thumb.trim_start_matches("data:image/png;base64,"))
            .unwrap();
        let small = image::load_from_memory(&bytes).unwrap();
        assert_eq!(small.height(), THUMB_HEIGHT);
    }
}
