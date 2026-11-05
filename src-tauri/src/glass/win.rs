//! The wallpaper settings for a screen, through Windows' `IDesktopWallpaper`.

use windows::{
    core::{HSTRING, PWSTR},
    Win32::{
        System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL},
        UI::Shell::{
            DesktopWallpaper, IDesktopWallpaper, DWPOS_CENTER, DWPOS_FIT, DWPOS_SPAN, DWPOS_STRETCH,
            DWPOS_TILE,
        },
    },
};

use super::Fit;
use crate::{
    error::{AppError, AppResult},
    shell::placement::Rect,
};

/// What Windows shows on one screen.
pub struct Wallpaper {
    /// The picture file; `None` for a solid colour.
    pub path: Option<String>,
    pub fit: Fit,
    pub background: [u8; 3],
}

/// Take a string COM allocated, then free it.
fn take(p: PWSTR) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is a valid, NUL-terminated string from COM that we own and free once.
    unsafe {
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

/// The wallpaper on the screen covering `screen` (the primary screen if none matches).
pub fn read(screen: Rect) -> AppResult<Wallpaper> {
    crate::apps::win::com_init();
    // SAFETY: COM calls on a live IDesktopWallpaper; returned strings are freed by `take`.
    unsafe {
        let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)
            .map_err(|e| AppError::Platform(format!("wallpaper service unavailable: {e}")))?;
        let count = desktop.GetMonitorDevicePathCount()?;
        let mut chosen: Option<String> = None;
        for i in 0..count {
            let Some(id) = desktop.GetMonitorDevicePathAt(i).ok().and_then(take) else {
                continue;
            };
            // Screens that are known but switched off have no rectangle.
            let Ok(rect) = desktop.GetMonitorRECT(&HSTRING::from(id.as_str())) else {
                continue;
            };
            let exact = rect.left == screen.x
                && rect.top == screen.y
                && rect.right - rect.left == screen.width as i32
                && rect.bottom - rect.top == screen.height as i32;
            if exact || (chosen.is_none() && rect.left == 0 && rect.top == 0) {
                chosen = Some(id);
                if exact {
                    break;
                }
            }
        }
        let path = chosen
            .as_deref()
            .and_then(|id| desktop.GetWallpaper(&HSTRING::from(id)).ok())
            .and_then(take)
            .filter(|p| !p.is_empty() && std::path::Path::new(p).exists());
        let fit = match desktop.GetPosition()? {
            DWPOS_CENTER => Fit::Center,
            DWPOS_TILE => Fit::Tile,
            DWPOS_STRETCH => Fit::Stretch,
            DWPOS_FIT => Fit::Contain,
            DWPOS_SPAN => Fit::Span,
            _ => Fit::Fill,
        };
        // COLORREF is 0x00BBGGRR.
        let color = desktop.GetBackgroundColor()?.0;
        let background = [
            (color & 0xff) as u8,
            ((color >> 8) & 0xff) as u8,
            ((color >> 16) & 0xff) as u8,
        ];
        Ok(Wallpaper {
            path,
            fit,
            background,
        })
    }
}
