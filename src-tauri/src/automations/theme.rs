//! Windows' light and dark mode: the two switches the Settings app flips, and the broadcast that
//! tells open apps to repaint.

use super::ThemeChoice;

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

#[cfg(windows)]
pub fn current() -> Option<ThemeChoice> {
    use windows::{
        core::{w, HSTRING},
        Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
    };
    let mut value = 0u32;
    let mut len = std::mem::size_of::<u32>() as u32;
    // SAFETY: reads one DWORD into a local of the right size.
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(KEY),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut len),
        )
    };
    read.is_ok().then_some(if value == 0 {
        ThemeChoice::Dark
    } else {
        ThemeChoice::Light
    })
}

/// Switch apps and Windows itself (taskbar, Start) to light or dark.
#[cfg(windows)]
pub fn apply(choice: ThemeChoice) -> crate::error::AppResult<()> {
    use windows::{
        core::{w, HSTRING},
        Win32::{
            Foundation::{LPARAM, WPARAM},
            System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD},
            UI::WindowsAndMessaging::{
                SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
            },
        },
    };
    let light: u32 = match choice {
        ThemeChoice::Light => 1,
        ThemeChoice::Dark => 0,
    };
    let key = HSTRING::from(KEY);
    for name in [w!("AppsUseLightTheme"), w!("SystemUsesLightTheme")] {
        // SAFETY: writes one DWORD from a local under the current user's own key.
        let written = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &key,
                name,
                REG_DWORD.0,
                Some((&light as *const u32).cast()),
                std::mem::size_of::<u32>() as u32,
            )
        };
        written
            .ok()
            .map_err(|e| crate::error::AppError::Platform(e.to_string()))?;
    }
    let area = w!("ImmersiveColorSet");
    // SAFETY: the standard "colour settings changed" broadcast; hung windows are skipped.
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(area.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn current() -> Option<ThemeChoice> {
    None
}

#[cfg(not(windows))]
pub fn apply(_choice: ThemeChoice) -> crate::error::AppResult<()> {
    Ok(())
}
