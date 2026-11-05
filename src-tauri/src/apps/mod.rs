//! Apps on the dock: the apps pinned to the Windows taskbar, in the taskbar's order, plus every
//! other app with an open window. Windows are grouped under their app the way the taskbar
//! groups them — by app id (AppUserModelID) when there is one, otherwise by program file.
//!
//! This file is the pure part (parsing, filtering, grouping); `win.rs` talks to Windows and
//! `service.rs` keeps the list current.

pub mod service;
#[cfg(windows)]
pub mod win;

use serde::Serialize;
use ts_rs::TS;

/// Event carrying the full app list whenever it changes.
pub const APPS_CHANGED: &str = "apps-changed";

/// How to start an app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// A shortcut file (keeps its arguments and start folder).
    Shortcut(String),
    /// An app id, opened through `shell:AppsFolder` (Store and packaged apps).
    AppId(String),
    /// A program file.
    Program(String),
}

/// An app pinned to the Windows taskbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub name: String,
    /// Its app id, when it has one.
    pub app_id: Option<String>,
    /// The program it starts, lowercased, when it is a plain program.
    pub exe: Option<String>,
    pub launch: Launch,
    /// Where its icon comes from (see `win::icon_png`).
    pub icon: String,
}

/// A top-level window that would show on the taskbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinInfo {
    pub hwnd: isize,
    pub title: String,
    /// The window's program, lowercased.
    pub exe: Option<String>,
    pub app_id: Option<String>,
    pub minimized: bool,
    /// A friendly name for its app, for apps that aren't pinned.
    pub app_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DockWindow {
    #[ts(type = "number")]
    pub hwnd: isize,
    pub title: String,
    pub active: bool,
    pub minimized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DockApp {
    /// Stable id for this app on the dock.
    pub key: String,
    pub name: String,
    pub pinned: bool,
    /// Icon source, passed back to `app_icon`.
    pub icon: String,
    /// Open windows, front-most first.
    pub windows: Vec<DockWindow>,
}

/// What the dock knows about a pin it can launch, by app key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launchable {
    pub key: String,
    pub launch: Launch,
}

// ---------- Pin order from the taskbar's own record ----------

/// One pinned item as the taskbar records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinRef {
    /// A shortcut in the "User Pinned\TaskBar" folder, by file name.
    Shortcut(String),
    /// A Store or packaged app, by app id.
    AppId(String),
}

/// The taskbar's pins, in order, read from `Taskband\Favorites`. That value is an undocumented
/// list of shell item ids; the shortcut file names and app ids appear in it as UTF-16 text in
/// pin order, at either byte alignment. `shortcuts` are the names of the files actually in the
/// pinned folder, so only real pins count. Without a readable record, the folder's shortcuts
/// are used in name order.
pub fn parse_taskband(blob: &[u8], shortcuts: &[String]) -> Vec<PinRef> {
    let mut found: Vec<(usize, PinRef)> = Vec::new();
    for name in shortcuts {
        let needle: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        if let Some(at) = blob.windows(needle.len()).position(|w| w == needle.as_slice()) {
            found.push((at, PinRef::Shortcut(name.clone())));
        }
    }
    for align in 0..2 {
        let units: Vec<u16> = blob
            .get(align..)
            .unwrap_or_default()
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        for (offset, id) in app_ids_in(&units) {
            if !found.iter().any(|(_, r)| *r == PinRef::AppId(id.clone())) {
                found.push((align + offset * 2, PinRef::AppId(id)));
            }
        }
    }
    if found.is_empty() {
        let mut names = shortcuts.to_vec();
        names.sort_by_key(|n| n.to_lowercase());
        return names.into_iter().map(PinRef::Shortcut).collect();
    }
    found.sort_by_key(|(at, _)| *at);
    found.into_iter().map(|(_, r)| r).collect()
}

/// App ids (`Publisher.Name_<13-char publisher hash>!App`) in a run of UTF-16 units, with the
/// unit offset each starts at.
fn app_ids_in(units: &[u16]) -> Vec<(usize, String)> {
    let allowed = |u: u16| {
        char::from_u32(u as u32).is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    let mut ids = Vec::new();
    for (bang, _) in units.iter().enumerate().filter(|(_, &u)| u == u16::from(b'!')) {
        let start = (0..bang).rev().take_while(|&i| allowed(units[i])).last();
        let end = (bang + 1..units.len()).take_while(|&i| allowed(units[i])).last();
        let (Some(start), Some(end)) = (start, end) else {
            continue;
        };
        let family = String::from_utf16_lossy(&units[start..bang]);
        let app = String::from_utf16_lossy(&units[bang + 1..=end]);
        // The package family name ends in "_" + a 13-character publisher hash.
        let hash_ok = family.rsplit_once('_').is_some_and(|(name, hash)| {
            !name.is_empty()
                && hash.len() == 13
                && hash.chars().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
        });
        if hash_ok {
            ids.push((start, format!("{family}!{app}")));
        }
    }
    ids
}

// ---------- Which windows belong on the taskbar ----------

pub const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
pub const WS_EX_APPWINDOW: u32 = 0x0004_0000;
pub const WS_EX_NOACTIVATE: u32 = 0x0800_0000;

/// What decides whether a top-level window gets a taskbar button.
#[derive(Debug, Clone, Copy)]
pub struct WindowTraits {
    pub visible: bool,
    pub has_owner: bool,
    pub ex_style: u32,
    /// Hidden by the system: on another virtual desktop, or a suspended Store app.
    pub cloaked: bool,
    pub has_title: bool,
    pub ours: bool,
}

/// The taskbar's own rule: visible, unowned, not cloaked, and not a tool or no-activate window
/// unless it explicitly asks for a button.
pub fn shows_on_taskbar(w: WindowTraits) -> bool {
    let asks = w.ex_style & WS_EX_APPWINDOW != 0;
    let hidden_style = w.ex_style & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) != 0;
    w.visible && !w.cloaked && w.has_title && !w.ours && (asks || (!w.has_owner && !hidden_style))
}

// ---------- Grouping ----------

/// App ids Windows gives some programs without the program setting one. File Explorer's pin
/// points at a shell location rather than a program, so its windows only match it by this id.
pub fn implicit_app_id(exe: &str) -> Option<&'static str> {
    exe.to_lowercase()
        .ends_with(r"\windows\explorer.exe")
        .then_some("Microsoft.Windows.Explorer")
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Whether a window belongs to a pin: same app id, or — when either has no app id — same program.
fn belongs(pin: &Pin, window: &WinInfo) -> bool {
    match (&pin.app_id, &window.app_id) {
        (Some(p), Some(w)) => same(p, w),
        _ => matches!((&pin.exe, &window.exe), (Some(p), Some(w)) if same(p, w)),
    }
}

fn pin_key(pin: &Pin) -> String {
    match &pin.launch {
        Launch::Shortcut(path) | Launch::Program(path) => format!("pin:{}", path.to_lowercase()),
        Launch::AppId(id) => format!("pin:{}", id.to_lowercase()),
    }
}

/// Pins first (in taskbar order), then other apps with windows in the order their windows were
/// found (front-most first). `foreground` marks the active window.
pub fn build(pins: &[Pin], windows: &[WinInfo], foreground: isize) -> (Vec<DockApp>, Vec<Launchable>) {
    let dock_window = |w: &WinInfo| DockWindow {
        hwnd: w.hwnd,
        title: w.title.clone(),
        active: w.hwnd == foreground,
        minimized: w.minimized,
    };
    let mut claimed = vec![false; windows.len()];
    let mut apps = Vec::new();
    let mut launchables = Vec::new();
    for pin in pins {
        let key = pin_key(pin);
        let mut mine = Vec::new();
        for (i, window) in windows.iter().enumerate() {
            if !claimed[i] && belongs(pin, window) {
                claimed[i] = true;
                mine.push(dock_window(window));
            }
        }
        launchables.push(Launchable {
            key: key.clone(),
            launch: pin.launch.clone(),
        });
        apps.push(DockApp {
            key,
            name: pin.name.clone(),
            pinned: true,
            icon: pin.icon.clone(),
            windows: mine,
        });
    }
    for (i, window) in windows.iter().enumerate() {
        if claimed[i] {
            continue;
        }
        let (key, icon, launch) = match (&window.app_id, &window.exe) {
            (Some(id), _) => (
                format!("app:{}", id.to_lowercase()),
                format!("aumid:{id}"),
                Some(Launch::AppId(id.clone())),
            ),
            (None, Some(exe)) => (
                format!("exe:{exe}"),
                format!("exe:{exe}"),
                Some(Launch::Program(exe.clone())),
            ),
            (None, None) => (format!("win:{}", window.hwnd), String::new(), None),
        };
        if let Some(app) = apps.iter_mut().find(|a| a.key == key) {
            app.windows.push(dock_window(window));
            continue;
        }
        if let Some(launch) = launch {
            launchables.push(Launchable {
                key: key.clone(),
                launch,
            });
        }
        apps.push(DockApp {
            key,
            name: window.app_name.clone(),
            pinned: false,
            icon,
            windows: vec![dock_window(window)],
        });
    }
    (apps, launchables)
}

/// What a click on an app does, the way the taskbar behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickAction {
    Launch,
    Activate(isize),
    Minimize(isize),
}

/// No windows: launch. The active window: minimize it — or, with several windows, move on to
/// the next one. Otherwise bring the front-most window forward.
pub fn click_action(app: &DockApp) -> ClickAction {
    let Some(active) = app.windows.iter().position(|w| w.active) else {
        return match app.windows.first() {
            Some(w) => ClickAction::Activate(w.hwnd),
            None => ClickAction::Launch,
        };
    };
    if app.windows.len() == 1 {
        ClickAction::Minimize(app.windows[active].hwnd)
    } else {
        ClickAction::Activate(app.windows[(active + 1) % app.windows.len()].hwnd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn taskband_order_mixes_shortcuts_and_app_ids_at_both_alignments() {
        let mut blob = vec![0x14, 0x00, 0x1f];
        blob.extend(utf16("Google Chrome.lnk"));
        blob.extend([0x00, 0x31, 0x9a]); // odd padding shifts the next text by one byte
        blob.extend(utf16("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"));
        blob.extend([0x00, 0x00]);
        blob.extend(utf16("File Explorer.lnk"));
        blob.extend([0x07]);
        blob.extend(utf16("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify")); // repeated: kept once
        let shortcuts = ["File Explorer.lnk".to_string(), "Google Chrome.lnk".to_string()];
        assert_eq!(
            parse_taskband(&blob, &shortcuts),
            vec![
                PinRef::Shortcut("Google Chrome.lnk".into()),
                PinRef::AppId("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify".into()),
                PinRef::Shortcut("File Explorer.lnk".into()),
            ]
        );
    }

    #[test]
    fn shortcuts_no_longer_in_the_folder_are_not_pins() {
        let blob = utf16("Old App.lnk");
        assert_eq!(
            parse_taskband(&blob, &["New.lnk".to_string()]),
            vec![PinRef::Shortcut("New.lnk".into())]
        );
    }

    #[test]
    fn without_a_record_the_folder_is_used_in_name_order() {
        let shortcuts = ["b.lnk".to_string(), "A.lnk".to_string()];
        assert_eq!(
            parse_taskband(&[], &shortcuts),
            vec![PinRef::Shortcut("A.lnk".into()), PinRef::Shortcut("b.lnk".into())]
        );
    }

    #[test]
    fn text_that_only_looks_like_an_app_id_is_ignored() {
        let blob = utf16("Hello!World and Name_tooshort!App");
        assert!(parse_taskband(&blob, &[]).is_empty());
    }

    fn traits() -> WindowTraits {
        WindowTraits {
            visible: true,
            has_owner: false,
            ex_style: 0,
            cloaked: false,
            has_title: true,
            ours: false,
        }
    }

    #[test]
    fn taskbar_rule() {
        assert!(shows_on_taskbar(traits()));
        assert!(!shows_on_taskbar(WindowTraits {
            visible: false,
            ..traits()
        }));
        assert!(!shows_on_taskbar(WindowTraits {
            cloaked: true,
            ..traits()
        }));
        assert!(!shows_on_taskbar(WindowTraits {
            has_title: false,
            ..traits()
        }));
        assert!(!shows_on_taskbar(WindowTraits {
            ours: true,
            ..traits()
        }));
        assert!(!shows_on_taskbar(WindowTraits {
            has_owner: true,
            ..traits()
        }));
        assert!(!shows_on_taskbar(WindowTraits {
            ex_style: WS_EX_TOOLWINDOW,
            ..traits()
        }));
        assert!(!shows_on_taskbar(WindowTraits {
            ex_style: WS_EX_NOACTIVATE,
            ..traits()
        }));
        // Asking for a button wins over owner and tool style.
        assert!(shows_on_taskbar(WindowTraits {
            has_owner: true,
            ex_style: WS_EX_TOOLWINDOW | WS_EX_APPWINDOW,
            ..traits()
        }));
    }

    fn pin(name: &str, app_id: Option<&str>, exe: Option<&str>) -> Pin {
        Pin {
            name: name.into(),
            app_id: app_id.map(Into::into),
            exe: exe.map(Into::into),
            launch: Launch::Shortcut(format!("C:\\pins\\{name}.lnk")),
            icon: format!("lnk:{name}"),
        }
    }

    fn window(hwnd: isize, app_id: Option<&str>, exe: Option<&str>) -> WinInfo {
        WinInfo {
            hwnd,
            title: format!("window {hwnd}"),
            exe: exe.map(Into::into),
            app_id: app_id.map(Into::into),
            minimized: false,
            app_name: format!("app of {hwnd}"),
        }
    }

    #[test]
    fn windows_group_under_pins_by_app_id_or_program() {
        let pins = [
            pin("Chrome", Some("Chrome"), Some("c:\\chrome.exe")),
            pin("Notepad", None, Some("c:\\notepad.exe")),
        ];
        let windows = [
            window(1, Some("chrome"), Some("c:\\chrome.exe")),
            window(2, None, Some("C:\\Notepad.exe")),
            window(3, Some("Chrome.Profile2"), Some("c:\\chrome.exe")), // another profile: its own app
            window(4, Some("Chrome"), Some("c:\\chrome.exe")),
        ];
        let (apps, _) = build(&pins, &windows, 4);
        let ids: Vec<Vec<isize>> = apps
            .iter()
            .map(|a| a.windows.iter().map(|w| w.hwnd).collect())
            .collect();
        assert_eq!(ids, vec![vec![1, 4], vec![2], vec![3]]);
        assert!(apps[0].windows[1].active);
        assert!(!apps[2].pinned);
        assert_eq!(apps[2].key, "app:chrome.profile2");
        assert_eq!(apps[2].icon, "aumid:Chrome.Profile2");
    }

    #[test]
    fn explorer_windows_join_the_file_explorer_pin() {
        assert_eq!(
            implicit_app_id(r"c:\windows\explorer.exe"),
            Some("Microsoft.Windows.Explorer")
        );
        assert_eq!(implicit_app_id(r"c:\tools\explorer.exe.bak"), None);
        // The pin is a shell location: an app id and no program.
        let pins = [Pin {
            launch: Launch::Shortcut(r"C:\pins\File Explorer.lnk".into()),
            ..pin("File Explorer", Some("Microsoft.Windows.Explorer"), None)
        }];
        let windows = [window(
            7,
            implicit_app_id(r"c:\windows\explorer.exe"),
            Some(r"c:\windows\explorer.exe"),
        )];
        let (apps, _) = build(&pins, &windows, 7);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].windows[0].hwnd, 7);
    }

    #[test]
    fn unpinned_windows_of_one_program_share_an_app() {
        let windows = [
            window(1, None, Some("c:\\code.exe")),
            window(2, None, Some("c:\\code.exe")),
        ];
        let (apps, launchables) = build(&[], &windows, 0);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].windows.len(), 2);
        assert_eq!(launchables[0].launch, Launch::Program("c:\\code.exe".into()));
    }

    #[test]
    fn pins_without_windows_stay_launchable() {
        let (apps, launchables) = build(&[pin("Spotify", Some("Spotify!App"), None)], &[], 0);
        assert!(apps[0].windows.is_empty());
        assert_eq!(launchables[0].key, apps[0].key);
    }

    fn app_with(active: &[bool]) -> DockApp {
        DockApp {
            key: "k".into(),
            name: "n".into(),
            pinned: true,
            icon: String::new(),
            windows: active
                .iter()
                .enumerate()
                .map(|(i, &active)| DockWindow {
                    hwnd: i as isize + 10,
                    title: String::new(),
                    active,
                    minimized: false,
                })
                .collect(),
        }
    }

    #[test]
    fn clicking_behaves_like_the_taskbar() {
        assert_eq!(click_action(&app_with(&[])), ClickAction::Launch);
        assert_eq!(click_action(&app_with(&[false])), ClickAction::Activate(10));
        assert_eq!(click_action(&app_with(&[true])), ClickAction::Minimize(10));
        assert_eq!(
            click_action(&app_with(&[false, true, false])),
            ClickAction::Activate(12)
        );
        assert_eq!(
            click_action(&app_with(&[false, false, true])),
            ClickAction::Activate(10)
        );
    }
}
