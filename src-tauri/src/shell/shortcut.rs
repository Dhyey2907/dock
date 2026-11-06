//! Global keyboard shortcuts: show or hide the dock, the command bar, quick capture. All are
//! registered together, so changing one never leaves another unregistered.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use ts_rs::TS;

use crate::{
    error::{AppError, AppResult},
    settings::Settings,
};

/// Event sent to every window when the quick capture shortcut is pressed.
/// Dock's own shortcuts registered now (the ones the hotkey editor sets).
static PLANNED: Mutex<Vec<Shortcut>> = Mutex::new(Vec::new());
/// Whether Alt+1 … Alt+9 are registered.
static NUMBERS: Mutex<bool> = Mutex::new(false);


/// Something a global shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HotkeyAction {
    /// Show or hide the dock.
    Toggle,
    CommandBar,
    QuickCapture,
}

impl HotkeyAction {
    pub const ALL: [HotkeyAction; 3] = [
        HotkeyAction::Toggle,
        HotkeyAction::CommandBar,
        HotkeyAction::QuickCapture,
    ];

    /// This action's shortcut text in `settings`.
    pub fn get(self, settings: &Settings) -> &str {
        match self {
            HotkeyAction::Toggle => &settings.toggle_shortcut,
            HotkeyAction::CommandBar => &settings.hotkeys.command_bar,
            HotkeyAction::QuickCapture => &settings.hotkeys.quick_capture,
        }
    }

    pub fn set(self, settings: &mut Settings, text: String) {
        match self {
            HotkeyAction::Toggle => settings.toggle_shortcut = text,
            HotkeyAction::CommandBar => settings.hotkeys.command_bar = text,
            HotkeyAction::QuickCapture => settings.hotkeys.quick_capture = text,
        }
    }

    fn run(self, app: &AppHandle) {
        match self {
            HotkeyAction::Toggle => super::toggle_dock(app),
            HotkeyAction::CommandBar => {
                // Off the event loop's thread: the first open creates a window.
                let app = app.clone();
                std::thread::spawn(move || {
                    if let Err(error) = crate::command::toggle_window(&app) {
                        tracing::warn!(%error, "could not open the command bar");
                    }
                });
            }
            HotkeyAction::QuickCapture => {
                let app = app.clone();
                // Off the main thread: the popup window may need creating.
                std::thread::spawn(move || {
                    if let Err(error) = super::panel::show_capture(&app) {
                        tracing::warn!(%error, "could not open quick capture");
                    }
                });
            }
        }
    }
}

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

/// Every action's shortcut, parsed and checked: none used twice.
pub fn plan(settings: &Settings) -> AppResult<Vec<(HotkeyAction, Shortcut)>> {
    let mut planned: Vec<(HotkeyAction, Shortcut)> = Vec::new();
    for action in HotkeyAction::ALL {
        let Some(shortcut) = parse(action.get(settings))? else {
            continue;
        };
        if planned.iter().any(|(_, s)| s.id() == shortcut.id()) {
            return Err(AppError::InvalidInput(
                "That shortcut is already used for something else in Dock.".into(),
            ));
        }
        planned.push((action, shortcut));
    }
    Ok(planned)
}

/// Register every shortcut in `settings`, replacing the ones registered before.
pub fn apply_all(app: &AppHandle, settings: &Settings) -> AppResult<()> {
    let planned = plan(settings)?;
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|e| AppError::Platform(e.to_string()))?;
    // Clearing everything cleared Alt+1 … Alt+9 too.
    *NUMBERS.lock().unwrap_or_else(|e| e.into_inner()) = false;
    *PLANNED.lock().unwrap_or_else(|e| e.into_inner()) = planned.iter().map(|(_, s)| *s).collect();
    let mut taken = None;
    for (action, shortcut) in planned {
        let result = manager.on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                action.run(app);
            }
        });
        if let Err(error) = result {
            tracing::warn!(?action, %shortcut, %error, "a shortcut is taken by another app");
            taken = Some(action);
        }
    }
    if let Err(error) = set_numbers(app, settings.apps.number_shortcuts) {
        tracing::warn!(%error, "Alt+number shortcuts not set");
    }
    match taken {
        Some(_) => Err(AppError::InvalidInput(
            "Another app already uses that shortcut.".into(),
        )),
        None => Ok(()),
    }
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
        // Never take the keys of one of Dock's own shortcuts.
        if PLANNED.lock().unwrap_or_else(|e| e.into_inner()).contains(&shortcut) {
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
    use super::*;

    #[test]
    fn shortcuts_need_a_modifier_and_a_real_key() {
        assert!(parse("Ctrl+Alt+D").unwrap().is_some());
        let numbers = number_shortcuts();
        assert_eq!(numbers.len(), 9);
        assert_eq!(numbers[8], (9, parse("Alt+9").unwrap().unwrap()));
        assert!(parse("Shift+Super+Space").unwrap().is_some());
        assert!(parse("  ").unwrap().is_none());
        assert!(parse("D").is_err());
        assert!(parse("Ctrl+Banana").is_err());
    }

    #[test]
    fn every_action_gets_its_own_shortcut() {
        let mut settings = Settings::default();
        assert_eq!(plan(&settings).unwrap().len(), 3);
        settings.hotkeys.quick_capture = String::new();
        assert_eq!(plan(&settings).unwrap().len(), 2);
        settings.hotkeys.quick_capture = "ctrl+alt+d".into();
        assert!(plan(&settings).is_err());
    }
}
