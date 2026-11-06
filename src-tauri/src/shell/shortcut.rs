//! Global keyboard shortcuts: show or hide the dock, the command bar, quick capture. All are
//! registered together, so changing one never leaves another unregistered.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use ts_rs::TS;

use crate::{
    error::{AppError, AppResult},
    settings::Settings,
};

/// Event sent to every window when the quick capture shortcut is pressed.
pub const QUICK_CAPTURE: &str = "quick-capture";

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
                if let Err(error) = app.emit(QUICK_CAPTURE, ()) {
                    tracing::warn!(%error, "could not start quick capture");
                }
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
    let mut taken = None;
    for (action, shortcut) in planned {
        let result = manager.on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                action.run(app);
            }
        });
        if result.is_err() {
            taken = Some(action);
        }
    }
    match taken {
        Some(_) => Err(AppError::InvalidInput(
            "Another app already uses that shortcut.".into(),
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_need_a_modifier_and_a_real_key() {
        assert!(parse("Ctrl+Alt+D").unwrap().is_some());
        assert!(parse("Shift+Super+Space").unwrap().is_some());
        assert!(parse("  ").unwrap().is_none());
        assert!(parse("D").is_err());
        assert!(parse("Ctrl+Banana").is_err());
    }

    #[test]
    fn every_action_gets_its_own_shortcut() {
        let mut settings = Settings::default();
        assert_eq!(plan(&settings).unwrap().len(), 2);
        settings.hotkeys.quick_capture = "Ctrl+Shift+N".into();
        assert_eq!(plan(&settings).unwrap().len(), 3);
        settings.hotkeys.quick_capture = "ctrl+alt+d".into();
        assert!(plan(&settings).is_err());
    }
}
