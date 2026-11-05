//! The keyboard shortcut that shows or hides the dock, from anywhere.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

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

/// Make `text` the show/hide shortcut, replacing the old one.
pub fn apply(app: &AppHandle, text: &str) -> AppResult<()> {
    let shortcut = parse(text)?;
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|e| AppError::Platform(e.to_string()))?;
    let Some(shortcut) = shortcut else { return Ok(()) };
    manager
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                super::toggle_dock(app);
            }
        })
        .map_err(|_| AppError::InvalidInput("Another app already uses that shortcut.".into()))
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
}
