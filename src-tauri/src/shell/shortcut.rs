//! The keyboard shortcut that shows or hides the dock, from anywhere.

use std::sync::Mutex;
use tauri::AppHandle;

use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::error::{AppError, AppResult};

/// Parse shortcut text as typed in Settings ("Ctrl+Alt+D"); empty means none.
pub fn parse(text: &str) -> AppResult<Option<Shortcut>> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let shortcut: Shortcut = text
        .parse()
        .map_err(|_| AppError::InvalidInput(format!("\"{text}\" isn't a shortcut Dock understands.")))?;
    // A plain key would fire while typing anywhere.
    if shortcut.mods.is_empty() {
        return Err(AppError::InvalidInput(
            "Use at least one of Ctrl, Alt, Shift or Win with the key.".into(),
        ));
    }
    Ok(Some(shortcut))
}

/// The show/hide shortcut registered now.
static CURRENT: Mutex<Option<Shortcut>> = Mutex::new(None);
/// Whether Alt+1 … Alt+9 are registered.
static NUMBERS: Mutex<bool> = Mutex::new(false);

/// Make `text` the show/hide shortcut, replacing the old one (and only that one: the
/// Alt+number shortcuts stay).
pub fn apply(app: &AppHandle, text: &str) -> AppResult<()> {
    let shortcut = parse(text)?;
    let manager = app.global_shortcut();
    let mut current = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = current.take() {
        manager
            .unregister(old)
            .map_err(|e| AppError::Platform(e.to_string()))?;
    }
    let Some(shortcut) = shortcut else { return Ok(()) };
    manager
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                super::toggle_dock(app);
            }
        })
        .map_err(|_| AppError::InvalidInput("Another app already uses that shortcut.".into()))?;
    *current = Some(shortcut);
    Ok(())
}

/// Alt+`n` for `n` in 1..=9.
pub fn number_shortcuts() -> Vec<(usize, Shortcut)> {
    const DIGITS: [Code; 9] = [
        Code::Digit1,
        Code::Digit2,
        Code::Digit3,
        Code::Digit4,
        Code::Digit5,
        Code::Digit6,
        Code::Digit7,
        Code::Digit8,
        Code::Digit9,
    ];
    DIGITS
        .iter()
        .enumerate()
        .map(|(i, &code)| (i + 1, Shortcut::new(Some(Modifiers::ALT), code)))
        .collect()
}

/// Turn Alt+1 … Alt+9 (open the pinned apps in order) on or off. A number another app
/// already uses is skipped.
pub fn set_numbers(app: &AppHandle, on: bool) -> AppResult<()> {
    let mut registered = NUMBERS.lock().unwrap_or_else(|e| e.into_inner());
    if *registered == on {
        return Ok(());
    }
    let manager = app.global_shortcut();
    for (n, shortcut) in number_shortcuts() {
        // Never take the show/hide shortcut's keys.
        if CURRENT.lock().unwrap_or_else(|e| e.into_inner()).as_ref() == Some(&shortcut) {
            continue;
        }
        let result = if on {
            manager.on_shortcut(shortcut, move |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    crate::apps::service::open_nth(app, n);
                }
            })
        } else {
            manager.unregister(shortcut)
        };
        if let Err(error) = result {
            tracing::debug!(%error, n, "Alt+number shortcut skipped");
        }
    }
    *registered = on;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn shortcuts_need_a_modifier_and_a_real_key() {
        assert!(parse("Ctrl+Alt+D").unwrap().is_some());
        assert!(parse("Shift+Super+Space").unwrap().is_some());
        assert!(parse("  ").unwrap().is_none());
        assert!(parse("D").is_err());
        assert!(parse("Ctrl+Banana").is_err());
    }

    #[test]
    fn alt_numbers_cover_one_to_nine() {
        let numbers = super::number_shortcuts();
        assert_eq!(numbers.len(), 9);
        assert_eq!(numbers[0].1, parse("Alt+1").unwrap().unwrap());
        assert_eq!(numbers[8], (9, parse("Alt+9").unwrap().unwrap()));
    }
}
