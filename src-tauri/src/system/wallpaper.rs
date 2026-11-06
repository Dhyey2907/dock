//! Wallpaper changer: pictures from a folder you pick, in turn, every few minutes or on a click.
//! Set through Windows' own desktop wallpaper object, on every screen.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

const PICTURES: [&str; 6] = ["jpg", "jpeg", "png", "bmp", "webp", "jfif"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WallpaperState {
    /// The folder pictures come from; empty when none is picked.
    pub folder: String,
    /// How many pictures it has.
    pub count: u32,
    /// The picture on the desktop now, by file name, if it's from the folder.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
}

pub fn is_picture(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PICTURES.contains(&e.to_ascii_lowercase().as_str()))
}

/// The pictures in `folder`, sorted by name (not its subfolders).
pub fn pictures(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_picture(p))
        .collect();
    found.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    found
}

/// The picture after `current` (the first one if `current` isn't in the list), wrapping round.
pub fn next_picture<'a>(pictures: &'a [PathBuf], current: Option<&Path>) -> Option<&'a PathBuf> {
    if pictures.is_empty() {
        return None;
    }
    let same =
        |p: &PathBuf| current.is_some_and(|c| p.to_string_lossy().eq_ignore_ascii_case(&c.to_string_lossy()));
    let next = match pictures.iter().position(same) {
        Some(i) => (i + 1) % pictures.len(),
        None => 0,
    };
    pictures.get(next)
}

/// The wallpaper on the first screen.
#[cfg(windows)]
pub fn current() -> Option<PathBuf> {
    with_wallpaper(|desktop| {
        // SAFETY: COM call on a live object; the returned string is freed below.
        unsafe {
            let text = desktop.GetWallpaper(windows::core::PCWSTR::null()).ok()?;
            let path = text.to_string().ok();
            windows::Win32::System::Com::CoTaskMemFree(Some(text.0 as *const core::ffi::c_void));
            path.filter(|p| !p.is_empty()).map(PathBuf::from)
        }
    })
    .flatten()
}

#[cfg(not(windows))]
pub fn current() -> Option<PathBuf> {
    None
}

/// Put `picture` on every screen.
#[cfg(windows)]
pub fn apply(picture: &Path) -> crate::error::AppResult<()> {
    let wide = super::to_wide(&picture.to_string_lossy());
    with_wallpaper(|desktop| {
        // SAFETY: COM call on a live object with a NUL-terminated path.
        unsafe {
            desktop.SetWallpaper(
                windows::core::PCWSTR::null(),
                windows::core::PCWSTR(wide.as_ptr()),
            )
        }
    })
    .ok_or_else(|| crate::error::AppError::Platform("desktop wallpaper unavailable".into()))?
    .map_err(Into::into)
}

#[cfg(not(windows))]
pub fn apply(_picture: &Path) -> crate::error::AppResult<()> {
    Ok(())
}

/// Run `f` with Windows' desktop wallpaper object, on this thread with COM started.
#[cfg(windows)]
fn with_wallpaper<T>(f: impl FnOnce(&windows::Win32::UI::Shell::IDesktopWallpaper) -> T) -> Option<T> {
    use windows::Win32::{
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{DesktopWallpaper, IDesktopWallpaper},
    };
    // SAFETY: COM is started and stopped on this thread around the object's whole life.
    unsafe {
        let started = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let result = CoCreateInstance::<_, IDesktopWallpaper>(&DesktopWallpaper, None, CLSCTX_LOCAL_SERVER)
            .ok()
            .map(|desktop| f(&desktop));
        if started {
            CoUninitialize();
        }
        result
    }
}

/// Move on to the next picture in `folder`. Returns its path.
pub fn advance(folder: &str) -> crate::error::AppResult<PathBuf> {
    let folder = folder.trim();
    if folder.is_empty() {
        return Err(crate::error::AppError::InvalidInput("no wallpaper folder".into()));
    }
    let list = pictures(Path::new(folder));
    let current = current();
    let next = next_picture(&list, current.as_deref())
        .ok_or_else(|| crate::error::AppError::InvalidInput("no pictures in the folder".into()))?
        .clone();
    apply(&next)?;
    Ok(next)
}

pub fn state(folder: &str) -> WallpaperState {
    let folder = folder.trim();
    let list = if folder.is_empty() {
        Vec::new()
    } else {
        pictures(Path::new(folder))
    };
    let current = current().filter(|c| {
        list.iter()
            .any(|p| p.to_string_lossy().eq_ignore_ascii_case(&c.to_string_lossy()))
    });
    WallpaperState {
        folder: folder.to_string(),
        count: list.len() as u32,
        current: current.and_then(|c| c.file_name().map(|n| n.to_string_lossy().into_owned())),
    }
}

/// Windows' folder picker. `None` when cancelled. Blocks until it closes, so call it off the
/// main thread.
#[cfg(windows)]
pub fn pick_folder(start: &str) -> Option<String> {
    use windows::Win32::{
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{
            FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName, FOS_FORCEFILESYSTEM,
            FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
        },
    };
    // SAFETY: COM is started and stopped on this thread around the dialog's whole life; the
    // path string is freed after copying.
    unsafe {
        let started = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let picked = (|| {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
            let options = dialog.GetOptions().ok()?;
            dialog
                .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)
                .ok()?;
            let _ = dialog.SetTitle(windows::core::w!("Choose a wallpaper folder"));
            if !start.trim().is_empty() {
                let wide = super::to_wide(start.trim());
                if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(
                    windows::core::PCWSTR(wide.as_ptr()),
                    None,
                ) {
                    let _ = dialog.SetFolder(&item);
                }
            }
            dialog.Show(None).ok()?;
            let item = dialog.GetResult().ok()?;
            let text = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let path = text.to_string().ok();
            CoTaskMemFree(Some(text.0 as *const core::ffi::c_void));
            path
        })();
        if started {
            CoUninitialize();
        }
        picked
    }
}

#[cfg(not(windows))]
pub fn pick_folder(_start: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_pictures_by_extension() {
        assert!(is_picture(Path::new(r"C:\w\a.JPG")));
        assert!(is_picture(Path::new("b.webp")));
        assert!(!is_picture(Path::new("notes.txt")));
        assert!(!is_picture(Path::new("jpg")));
    }

    #[test]
    fn steps_through_pictures_and_wraps() {
        let list: Vec<PathBuf> = ["a.jpg", "b.png", "c.jpg"].iter().map(PathBuf::from).collect();
        assert_eq!(next_picture(&list, None), Some(&list[0]));
        assert_eq!(next_picture(&list, Some(Path::new("A.JPG"))), Some(&list[1]));
        assert_eq!(next_picture(&list, Some(Path::new("c.jpg"))), Some(&list[0]));
        assert_eq!(
            next_picture(&list, Some(Path::new("elsewhere.jpg"))),
            Some(&list[0])
        );
        assert_eq!(next_picture(&[], None), None);
    }

    #[test]
    fn lists_only_pictures_in_the_folder_sorted() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["b.png", "A.jpg", "notes.txt"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub.jpg")).unwrap();
        let names: Vec<String> = pictures(dir.path())
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["A.jpg", "b.png"]);
        assert!(pictures(&dir.path().join("missing")).is_empty());
    }
}
