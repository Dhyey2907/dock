//! Which docks exist and which screen each one is on, and which dock a window belongs to.
//! Pure, so it is testable; `mod.rs` creates and removes the windows.
//!
//! The main dock is the window labelled `dock` (it comes from the app config and is never
//! destroyed). It sits on the screen chosen in settings, or the main screen, and carries the
//! pinned apps and the widgets. With "every screen", each other screen gets a plain dock
//! labelled `dock-<n>` (n from the screen's device name) showing just the windows on it.

use super::placement::Rect;
use crate::settings::DocksOn;

pub const MAIN_LABEL: &str = "dock";
const SECONDARY_PREFIX: &str = "dock-";

/// A connected screen, as far as the plan cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenId {
    /// Windows' device name ("\\.\DISPLAY2").
    pub name: String,
    pub primary: bool,
}

/// One dock that should exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockSlot {
    pub label: String,
    /// The device name of its screen.
    pub screen: String,
}

/// Whether a window label is one of the docks.
pub fn is_dock_label(label: &str) -> bool {
    label == MAIN_LABEL || label.starts_with(SECONDARY_PREFIX)
}

/// The trailing number of a device name ("\\.\DISPLAY12" → "12"), if it has one.
pub fn screen_number(name: &str) -> Option<&str> {
    let digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    (digits > 0).then(|| &name[name.len() - digits..])
}

/// A window label for a secondary dock on the screen `name`. Labels only allow a few
/// characters, so anything else in an unusual device name is dropped.
fn secondary_label(name: &str) -> String {
    let id: String = match screen_number(name) {
        Some(number) => number.to_string(),
        None => name.chars().filter(char::is_ascii_alphanumeric).collect(),
    };
    format!("{SECONDARY_PREFIX}{}", if id.is_empty() { "x" } else { &id })
}

/// The docks for the connected `screens`: the main dock on `chosen` (or the main screen if
/// `chosen` is empty or not connected), then — with `DocksOn::Every` — one on every other
/// screen, in screen order.
pub fn plan(screens: &[ScreenId], chosen: &str, docks_on: DocksOn) -> Vec<DockSlot> {
    let main_screen = screens
        .iter()
        .find(|s| !chosen.is_empty() && s.name == chosen)
        .or_else(|| screens.iter().find(|s| s.primary))
        .or(screens.first());
    let Some(main_screen) = main_screen else {
        return Vec::new();
    };
    let mut slots = vec![DockSlot {
        label: MAIN_LABEL.into(),
        screen: main_screen.name.clone(),
    }];
    if docks_on == DocksOn::Every {
        for screen in screens.iter().filter(|s| s.name != main_screen.name) {
            let mut label = secondary_label(&screen.name);
            // Two odd names could boil down to the same label.
            while slots.iter().any(|s| s.label == label) {
                label.push('x');
            }
            slots.push(DockSlot {
                label,
                screen: screen.name.clone(),
            });
        }
    }
    slots
}

/// The dock a window shows on: the dock whose screen is the window's screen (`window_screen`,
/// the full rectangle of the screen Windows says it's on). Windows on a screen without a dock,
/// or whose screen isn't known, show on the main dock. `docks` is (label, screen rectangle).
pub fn dock_for_window(window_screen: Option<Rect>, docks: &[(String, Rect)]) -> &str {
    window_screen
        .and_then(|screen| docks.iter().find(|(_, rect)| *rect == screen))
        .map_or(MAIN_LABEL, |(label, _)| label.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(name: &str, primary: bool) -> ScreenId {
        ScreenId {
            name: name.into(),
            primary,
        }
    }

    fn labels(slots: &[DockSlot]) -> Vec<(&str, &str)> {
        slots
            .iter()
            .map(|s| (s.label.as_str(), s.screen.as_str()))
            .collect()
    }

    const ONE: &str = r"\\.\DISPLAY1";
    const TWO: &str = r"\\.\DISPLAY2";

    #[test]
    fn one_dock_goes_on_the_chosen_screen_or_the_main_one() {
        let screens = [screen(ONE, true), screen(TWO, false)];
        assert_eq!(labels(&plan(&screens, "", DocksOn::One)), [("dock", ONE)]);
        assert_eq!(labels(&plan(&screens, TWO, DocksOn::One)), [("dock", TWO)]);
        // An unplugged choice falls back to the main screen.
        assert_eq!(
            labels(&plan(&screens, r"\\.\DISPLAY7", DocksOn::One)),
            [("dock", ONE)]
        );
    }

    #[test]
    fn every_screen_gets_a_dock_and_the_main_one_follows_the_choice() {
        let screens = [screen(ONE, true), screen(TWO, false)];
        assert_eq!(
            labels(&plan(&screens, "", DocksOn::Every)),
            [("dock", ONE), ("dock-2", TWO)]
        );
        assert_eq!(
            labels(&plan(&screens, TWO, DocksOn::Every)),
            [("dock", TWO), ("dock-1", ONE)]
        );
    }

    #[test]
    fn no_screens_means_no_docks_and_odd_names_still_get_labels() {
        assert!(plan(&[], "", DocksOn::Every).is_empty());
        let screens = [
            screen("main", true),
            screen("Proj A", false),
            screen("Proj-A", false),
        ];
        assert_eq!(
            labels(&plan(&screens, "", DocksOn::Every)),
            [
                ("dock", "main"),
                ("dock-ProjA", "Proj A"),
                ("dock-ProjAx", "Proj-A")
            ]
        );
    }

    #[test]
    fn screen_numbers_come_from_the_end_of_the_name() {
        assert_eq!(screen_number(r"\\.\DISPLAY12"), Some("12"));
        assert_eq!(screen_number("main"), None);
        assert!(is_dock_label("dock") && is_dock_label("dock-2"));
        assert!(!is_dock_label("panel") && !is_dock_label("docker"));
    }

    #[test]
    fn windows_go_to_the_dock_on_their_screen() {
        let left = Rect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1600,
        };
        let right = Rect {
            x: 2560,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let docks = vec![("dock".to_string(), left), ("dock-2".to_string(), right)];
        assert_eq!(dock_for_window(Some(right), &docks), "dock-2");
        assert_eq!(dock_for_window(Some(left), &docks), "dock");
        // Unknown screen, or one without a dock: the main dock.
        assert_eq!(dock_for_window(None, &docks), "dock");
        let third = Rect { x: -1920, ..right };
        assert_eq!(dock_for_window(Some(third), &docks), "dock");
        // With only the main dock, everything is on it, wherever it is.
        let main_only = vec![("dock".to_string(), right)];
        assert_eq!(dock_for_window(Some(left), &main_only), "dock");
    }
}
