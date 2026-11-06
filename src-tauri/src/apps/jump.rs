//! Jump lists: what an app's right-click menu shows above its windows, like the taskbar's.
//! File Explorer gets its pinned and frequent folders (Quick access); other apps get the
//! frequent and recent items Windows keeps for them. Apps' own custom lists ("New incognito
//! window") have no public way to be read, so they aren't shown.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How many items each section shows at most.
const SECTION_LIMIT: usize = 9;

/// Explorer's app id (see `implicit_app_id`).
pub const EXPLORER_APP_ID: &str = "Microsoft.Windows.Explorer";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JumpItem {
    pub name: String,
    /// What opening it opens: a folder or file path.
    pub path: String,
    /// Icon source for `app_icon` ("path:<path>").
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JumpSection {
    /// "Pinned", "Frequent" or "Recent".
    pub title: String,
    pub items: Vec<JumpItem>,
}

/// An app's right-click menu, shown in the popup window.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppMenu {
    /// The dock it was opened from, and the app's key on it.
    pub dock: String,
    pub key: String,
    pub name: String,
    /// Icon source for `app_icon`.
    pub icon: String,
    /// Whether the app can be started again ("open a new window").
    pub launchable: bool,
    pub windows: Vec<super::DockWindow>,
    pub sections: Vec<JumpSection>,
    pub kind: super::ItemKind,
    pub pinned: bool,
    /// One of the dock's own pins (a website or folder): it can be unpinned here.
    pub custom: bool,
    pub divider_after: bool,
    /// One of its windows isn't responding: offer to force it closed.
    pub hung: bool,
    /// It has windows whose programs can be ended ("End task", as on the Windows 11 taskbar).
    pub can_end_task: bool,
    /// There is another screen to move its window to.
    pub can_move: bool,
    /// It has windows and can be started again.
    pub can_restart: bool,
    /// It has a name or icon of the user's.
    pub customised: bool,
    /// What it is playing, when it is the app playing.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<MenuMedia>,
    /// The user's "open with arguments" entries for it.
    pub presets: Vec<NamedItem>,
    /// The user's own buttons for it.
    pub quick: Vec<NamedItem>,
}

/// An entry the user named in settings, by id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NamedItem {
    pub id: String,
    pub name: String,
}

/// The playing track, for media buttons in the app's menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MenuMedia {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub can_play_pause: bool,
    pub can_next: bool,
    pub can_previous: bool,
}

/// A media button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MediaKey {
    PlayPause,
    Next,
    Previous,
}

/// A choice in an app's menu.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum MenuAction {
    /// Bring one of its windows forward.
    Window {
        #[ts(type = "number")]
        hwnd: isize,
    },
    /// Open a jump list item.
    Open { path: String },
    /// Start the app again.
    Launch,
    /// Close all its windows.
    Close,
    /// Play/pause, next or previous, for the app that is playing.
    Media { key: MediaKey },
    /// Close it and start it again.
    Restart,
    /// Force closed a program that stopped responding (the page asks first).
    ForceClose,
    /// End the app's programs at once, frozen or not (the page asks first).
    EndTask,
    /// Move its front window to the next screen.
    MoveToScreen,
    /// Start it with one of the user's argument presets.
    Preset { id: String },
    /// One of the user's own buttons.
    Quick { id: String },
    /// Give it a name of the user's; empty goes back to its own.
    Rename { name: String },
    /// Pick an image for its icon.
    ChangeIcon,
    /// Back to its own name and icon.
    ResetLook,
    /// Never show it on the dock.
    Hide,
    /// Put a divider after it, or take it away.
    ToggleDivider,
    /// Remove one of the dock's own pins.
    Unpin,
}

impl AppMenu {
    /// The popup's size in logical pixels: rows of items, section titles and dividers.
    pub fn size(&self) -> (f64, f64) {
        const ROW: f64 = 36.0;
        const TITLE: f64 = 30.0;
        const DIVIDER: f64 = 9.0;
        const PADDING: f64 = 12.0;
        const MEDIA: f64 = 76.0;
        const TOOLBAR: f64 = 44.0;
        let items: usize = self.sections.iter().map(|s| s.items.len()).sum();
        let mut height = 2.0 * PADDING + TITLE * self.sections.len() as f64 + ROW * items as f64;
        // Each group after the first is set off by a divider.
        let mut groups = usize::from(items > 0);
        let mut group = |height: &mut f64, size: f64| {
            if size > 0.0 {
                if groups > 0 {
                    *height += DIVIDER;
                }
                groups += 1;
                *height += size;
            }
        };
        if self.media.is_some() {
            group(&mut height, MEDIA);
        }
        group(&mut height, ROW * self.windows.len().min(MAX_WINDOWS) as f64);
        let actions = usize::from(self.launchable)
            + usize::from(!self.windows.is_empty())
            + usize::from(self.hung)
            + usize::from(self.can_end_task)
            + self.presets.len()
            + self.quick.len();
        group(&mut height, ROW * actions as f64);
        group(&mut height, TOOLBAR);
        (300.0, height.max(ROW + 2.0 * PADDING))
    }

    /// Whether `path` is one of the items this menu offered.
    pub fn offers(&self, path: &str) -> bool {
        self.sections
            .iter()
            .flat_map(|s| &s.items)
            .any(|i| i.path == path)
    }
}

/// Most windows listed in a menu.
pub const MAX_WINDOWS: usize = 8;

/// The sections for the app with `app_id`; empty when Windows keeps nothing for it.
pub fn sections(app_id: Option<&str>) -> Vec<JumpSection> {
    #[cfg(windows)]
    {
        let found = match app_id {
            Some(id) if id.eq_ignore_ascii_case(EXPLORER_APP_ID) => win::explorer_folders(),
            Some(id) => win::documents(id),
            None => Vec::new(),
        };
        found
            .into_iter()
            .map(|(title, items)| JumpSection {
                title: title.into(),
                items: items.into_iter().take(SECTION_LIMIT).collect(),
            })
            .filter(|s| !s.items.is_empty())
            .collect()
    }
    #[cfg(not(windows))]
    {
        let _ = app_id;
        Vec::new()
    }
}

fn item(name: String, path: String) -> JumpItem {
    JumpItem {
        icon: format!("path:{path}"),
        name,
        path,
    }
}

#[cfg(windows)]
mod win {
    use windows::{
        core::{Interface, HSTRING, PCWSTR},
        Win32::{
            System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER},
            UI::Shell::{
                ApplicationDocumentLists, BHID_EnumItems, Common::IObjectArray, IApplicationDocumentLists,
                IEnumShellItems, IShellItem, IShellItem2, PropertiesSystem::PSGetPropertyKeyFromName,
                SHCreateItemFromParsingName, ADLT_FREQUENT, ADLT_RECENT, SIGDN_DESKTOPABSOLUTEPARSING,
                SIGDN_NORMALDISPLAY,
            },
        },
    };

    use super::{item, JumpItem};

    type Found = Vec<(&'static str, Vec<JumpItem>)>;

    /// A shell item's display name and parsing path (`None` for things with no path).
    fn describe(shell_item: &IShellItem) -> Option<JumpItem> {
        // SAFETY: COM calls on a live item; each returned string is copied, then freed.
        unsafe {
            let read = |sigdn| -> Option<String> {
                let raw = shell_item.GetDisplayName(sigdn).ok()?;
                let text = raw.to_string().ok();
                CoTaskMemFree(Some(raw.0 as *const _));
                text
            };
            let name = read(SIGDN_NORMALDISPLAY)?;
            let path = read(SIGDN_DESKTOPABSOLUTEPARSING)?;
            // Only real places: "::{GUID}" items (Recycle Bin, Home) aren't jump list targets.
            (!path.starts_with("::")).then(|| item(name, path))
        }
    }

    /// File Explorer's pinned and frequent folders, the same list its taskbar jump list shows.
    pub fn explorer_folders() -> Found {
        crate::apps::win::com_init();
        let mut pinned = Vec::new();
        let mut frequent = Vec::new();
        // SAFETY: COM calls on objects owned here; out-parameters are locals of the right type.
        unsafe {
            let Ok(folder) = SHCreateItemFromParsingName::<_, _, IShellItem>(
                &HSTRING::from("shell:::{3936E9E4-D92C-4EEE-A85A-BC16D5EA0819}"),
                None,
            ) else {
                return Vec::new();
            };
            let Ok(list) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else {
                return Vec::new();
            };
            let mut is_pinned_key = Default::default();
            let has_key = PSGetPropertyKeyFromName(
                PCWSTR(HSTRING::from("System.Home.IsPinned").as_ptr()),
                &mut is_pinned_key,
            )
            .is_ok();
            loop {
                let mut next = [None];
                let mut count = 0;
                if list.Next(&mut next, Some(&mut count)).is_err() || count == 0 {
                    break;
                }
                let Some(shell_item) = next[0].take() else { break };
                let Some(found) = describe(&shell_item) else {
                    continue;
                };
                let is_pinned = has_key
                    && shell_item
                        .cast::<IShellItem2>()
                        .and_then(|i| i.GetBool(&is_pinned_key))
                        .is_ok_and(|b| b.as_bool());
                if is_pinned {
                    pinned.push(found);
                } else {
                    frequent.push(found);
                }
            }
        }
        vec![("Pinned", pinned), ("Frequent", frequent)]
    }

    /// The frequent and recent items Windows keeps for `app_id`.
    pub fn documents(app_id: &str) -> Found {
        crate::apps::win::com_init();
        // SAFETY: COM calls on objects owned here.
        unsafe {
            let Ok(lists) = CoCreateInstance::<_, IApplicationDocumentLists>(
                &ApplicationDocumentLists,
                None,
                CLSCTX_INPROC_SERVER,
            ) else {
                return Vec::new();
            };
            if lists.SetAppID(&HSTRING::from(app_id)).is_err() {
                return Vec::new();
            }
            let read = |kind| -> Vec<JumpItem> {
                let Ok(array) = lists.GetList::<IObjectArray>(kind, super::SECTION_LIMIT as u32) else {
                    return Vec::new();
                };
                let count = array.GetCount().unwrap_or(0);
                (0..count)
                    .filter_map(|i| array.GetAt::<IShellItem>(i).ok())
                    .filter_map(|i| describe(&i))
                    .collect()
            };
            let frequent = read(ADLT_FREQUENT);
            // Recent, minus what Frequent already shows.
            let recent = read(ADLT_RECENT)
                .into_iter()
                .filter(|r| !frequent.iter().any(|f| f.path == r.path))
                .collect();
            vec![("Frequent", frequent), ("Recent", recent)]
        }
    }
}

// ---------- Stacks and the dock's own menu ----------

/// Most files a stack shows.
pub const STACK_LIMIT: usize = 24;
const STACK_COLUMNS: usize = 4;

/// A pinned folder's files, newest first, as its popup shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StackView {
    pub dock: String,
    pub key: String,
    pub name: String,
    pub path: String,
    pub items: Vec<JumpItem>,
}

/// One entry of a folder, as read from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    /// Seconds since some fixed point.
    pub modified: u64,
    pub hidden: bool,
}

/// A folder's entries for its stack: hidden ones left out, newest first, at most `STACK_LIMIT`.
pub fn stack_items(mut entries: Vec<FolderEntry>) -> Vec<JumpItem> {
    entries.retain(|e| !e.hidden && !e.name.starts_with('.') && !e.name.eq_ignore_ascii_case("desktop.ini"));
    entries.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    entries
        .into_iter()
        .take(STACK_LIMIT)
        .map(|e| JumpItem {
            icon: format!("path:{}", e.path),
            name: e.name,
            path: e.path,
        })
        .collect()
}

impl StackView {
    /// A grid of icons under a title, with a button row.
    pub fn size(&self) -> (f64, f64) {
        const CELL_W: f64 = 88.0;
        const CELL_H: f64 = 92.0;
        const HEAD: f64 = 44.0;
        const FOOT: f64 = 46.0;
        const PADDING: f64 = 12.0;
        let rows = self.items.len().div_ceil(STACK_COLUMNS).max(1);
        let columns = self.items.len().clamp(1, STACK_COLUMNS);
        (
            (CELL_W * columns as f64 + 2.0 * PADDING).max(260.0),
            HEAD + CELL_H * rows as f64 + FOOT + 2.0 * PADDING,
        )
    }

    pub fn offers(&self, path: &str) -> bool {
        self.path == path || self.items.iter().any(|i| i.path == path)
    }
}

/// The dock's own menu, from a right-click on an empty part of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DockMenu {
    pub dock: String,
    /// Apps closed recently, newest first.
    pub recent: Vec<super::history::RecentApp>,
    /// The user's own buttons for the dock.
    pub quick: Vec<NamedItem>,
    /// How many windows "Close all windows" would close.
    pub windows: usize,
}

impl DockMenu {
    pub fn size(&self) -> (f64, f64) {
        const ROW: f64 = 36.0;
        const TITLE: f64 = 30.0;
        const DIVIDER: f64 = 9.0;
        const PADDING: f64 = 12.0;
        // Show desktop, Minimize others, Close all, Task Manager, Dock settings.
        let mut height = 2.0 * PADDING + ROW * (4 + usize::from(self.windows > 0)) as f64;
        if !self.recent.is_empty() {
            height += DIVIDER + TITLE + ROW * self.recent.len() as f64;
        }
        if !self.quick.is_empty() {
            height += DIVIDER + ROW * self.quick.len() as f64;
        }
        (280.0, height)
    }
}

/// A choice in the dock's own menu.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum DockMenuAction {
    ShowDesktop,
    /// Minimize every window but the one in front.
    MinimizeOthers,
    /// Close every window on the docks (the page asks first).
    CloseAll,
    TaskManager,
    Settings,
    /// Start a recently closed app again.
    Reopen {
        key: String,
    },
    Quick {
        id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacks_show_visible_files_newest_first() {
        let entry = |name: &str, modified, hidden| FolderEntry {
            name: name.into(),
            path: format!(r"C:\d\{name}"),
            modified,
            hidden,
        };
        let items = stack_items(vec![
            entry("old.txt", 1, false),
            entry("new.png", 9, false),
            entry(".git", 10, false),
            entry("secret", 11, true),
            entry("desktop.ini", 12, false),
        ]);
        assert_eq!(
            items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            ["new.png", "old.txt"]
        );
        assert_eq!(items[0].icon, r"path:C:\d\new.png");
        let many = stack_items((0..40).map(|i| entry(&format!("f{i}"), i, false)).collect());
        assert_eq!(many.len(), STACK_LIMIT);
        let view = StackView {
            dock: "dock".into(),
            key: "stack:1".into(),
            name: "d".into(),
            path: r"C:\d".into(),
            items,
        };
        assert!(view.offers(r"C:\d") && view.offers(r"C:\d\old.txt") && !view.offers(r"C:\other"));
        assert!(view.size().0 >= 260.0);
    }

    #[test]
    fn dock_menu_grows_with_recent_apps() {
        let mut menu = DockMenu {
            dock: "dock".into(),
            recent: Vec::new(),
            quick: Vec::new(),
            windows: 0,
        };
        let empty = menu.size().1;
        menu.windows = 3;
        menu.recent = vec![super::super::history::RecentApp {
            key: "a".into(),
            name: "A".into(),
            icon: String::new(),
        }];
        assert_eq!(menu.size().1, empty + 36.0 + 9.0 + 30.0 + 36.0);
    }

    fn menu(items: usize, windows: usize, launchable: bool) -> AppMenu {
        AppMenu {
            dock: "dock".into(),
            key: "pin:x".into(),
            name: "X".into(),
            icon: String::new(),
            launchable,
            windows: (0..windows)
                .map(|i| super::super::DockWindow {
                    hwnd: i as isize,
                    title: format!("w{i}"),
                    active: false,
                    minimized: false,
                    hung: false,
                    flashing: false,
                })
                .collect(),
            sections: vec![JumpSection {
                title: "Pinned".into(),
                items: (0..items)
                    .map(|i| item(format!("i{i}"), format!("C:\\f{i}")))
                    .collect(),
            }],
            kind: super::super::ItemKind::App,
            pinned: true,
            custom: false,
            divider_after: false,
            hung: false,
            can_end_task: false,
            can_move: false,
            can_restart: false,
            customised: false,
            media: None,
            presets: Vec::new(),
            quick: Vec::new(),
        }
    }

    #[test]
    fn size_grows_with_rows() {
        let small = menu(0, 0, true).size().1;
        let big = menu(5, 2, true).size().1;
        assert!(big > small + 5.0 * 36.0);
        let mut more = menu(5, 2, true);
        more.media = Some(MenuMedia {
            title: "t".into(),
            artist: "a".into(),
            playing: true,
            can_play_pause: true,
            can_next: true,
            can_previous: true,
        });
        more.hung = true;
        more.presets = vec![NamedItem {
            id: "p".into(),
            name: "Work profile".into(),
        }];
        assert_eq!(more.size().1, big + 9.0 + 76.0 + 2.0 * 36.0);
    }

    #[test]
    fn end_task_takes_a_row_of_its_own() {
        let mut m = menu(0, 1, true);
        let without = m.size().1;
        m.can_end_task = true;
        assert_eq!(m.size().1, without + 36.0);
        m.hung = true;
        assert_eq!(m.size().1, without + 72.0);
    }

    #[test]
    fn only_offered_paths_open() {
        let m = menu(2, 0, true);
        assert!(m.offers("C:\\f1"));
        assert!(!m.offers("C:\\Windows\\System32\\cmd.exe"));
    }
}
