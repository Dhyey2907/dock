//! The Windows taskbar's corner, from the dock: its buttons (hidden icons, the input language,
//! network/volume/battery, the clock, notifications) open Windows' own flyouts. Each is pressed
//! on the real taskbar through UI Automation, which works while the taskbar is hidden, so they
//! behave exactly as on the taskbar; a Windows key shortcut is the fallback. A hidden taskbar
//! shows none of its buttons to UI Automation, so the dock finds them just before it hides the
//! taskbar (`remember`) and presses those.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A button in the taskbar's corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TrayButton {
    /// The ^ that shows the tray icons that don't fit.
    HiddenIcons,
    /// The input language (ENG IN), which lists keyboards to switch to.
    Language,
    /// Network, volume and battery: Quick Settings.
    QuickSettings,
    /// The clock: the calendar and notifications.
    Clock,
    /// The notification bell.
    Notifications,
}

impl TrayButton {
    /// How the button's UI Automation name starts on the taskbar (English Windows).
    fn names(self) -> &'static [&'static str] {
        match self {
            TrayButton::HiddenIcons => &["Show Hidden Icons"],
            TrayButton::Language => &["Tray Input Indicator"],
            TrayButton::QuickSettings => &["Network", "Volume", "Power", "Battery"],
            TrayButton::Clock => &["Clock"],
            TrayButton::Notifications => &["Notifications"],
        }
    }

    /// The Windows key shortcut that opens the same flyout (no shortcut shows hidden icons).
    fn shortcut(self) -> Option<char> {
        match self {
            TrayButton::HiddenIcons => None,
            TrayButton::Language => Some(' '),
            TrayButton::QuickSettings => Some('A'),
            TrayButton::Clock | TrayButton::Notifications => Some('N'),
        }
    }
}

/// The input language as the taskbar shows it: three letters and the region ("ENG", "IN"),
/// and whether there's more than one to switch between (the taskbar hides it otherwise).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InputLanguage {
    pub language: String,
    pub region: String,
    pub several: bool,
}

/// Windows' own look for the taskbar: its accent colour and whether the taskbar is light.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemLook {
    /// "#rrggbb".
    pub accent: String,
    /// The taskbar and Start are light ("Choose your mode: Light").
    pub light: bool,
}

/// Read Windows' accent colour and taskbar mode.
pub fn system_look() -> SystemLook {
    #[cfg(windows)]
    {
        let accent = win::dword(r"Software\Microsoft\Windows\DWM", "AccentColor")
            // Stored as 0xAABBGGRR.
            .map(|v| format!("#{:02x}{:02x}{:02x}", v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF))
            .unwrap_or_else(|| "#4cc2ff".into());
        let light = win::dword(
            r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "SystemUsesLightTheme",
        )
        .is_some_and(|v| v != 0);
        SystemLook { accent, light }
    }
    #[cfg(not(windows))]
    SystemLook {
        accent: "#4cc2ff".into(),
        light: false,
    }
}

/// The time and date as the taskbar writes them (Windows' short formats).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemClock {
    pub time: String,
    pub date: String,
}

pub fn system_clock() -> SystemClock {
    #[cfg(windows)]
    {
        win::clock()
    }
    #[cfg(not(windows))]
    {
        let now = chrono::Local::now();
        SystemClock {
            time: now.format("%H:%M").to_string(),
            date: now.format("%d-%m-%Y").to_string(),
        }
    }
}

/// Press `button` on the real taskbar, off the calling thread (UI Automation can be slow, and
/// needs its own COM setup).
pub fn press(button: TrayButton) {
    let _ = std::thread::Builder::new()
        .name("tray-button".into())
        .spawn(move || {
            #[cfg(windows)]
            if !win::invoke(button) {
                match button.shortcut() {
                    Some(key) => win::win_shortcut(key),
                    None => tracing::warn!(?button, "taskbar button not found"),
                }
            }
        });
}

#[cfg(windows)]
pub use win::{input_language, remember};

#[cfg(not(windows))]
pub fn remember() {}

#[cfg(not(windows))]
pub fn input_language() -> InputLanguage {
    InputLanguage {
        language: String::new(),
        region: String::new(),
        several: false,
    }
}

#[cfg(windows)]
mod win {
    use windows::{
        core::{w, PCWSTR},
        Win32::{
            Globalization::{
                GetLocaleInfoEx, LCIDToLocaleName, LOCALE_SISO3166CTRYNAME, LOCALE_SISO639LANGNAME2,
            },
            System::{
                Com::{
                    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                    COINIT_MULTITHREADED,
                },
                Variant::VARIANT,
            },
            UI::{
                Accessibility::{
                    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
                    TreeScope_Descendants, UIA_AutomationIdPropertyId, UIA_InvokePatternId,
                },
                Input::KeyboardAndMouse::{
                    GetKeyboardLayout, GetKeyboardLayoutList, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD,
                    KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_LWIN, VK_SPACE,
                },
                WindowsAndMessaging::{FindWindowW, GetForegroundWindow, GetWindowThreadProcessId},
            },
        },
    };

    use std::sync::Mutex;

    use super::{InputLanguage, TrayButton};

    /// Now, in the user's short time (no seconds) and short date formats.
    pub fn clock() -> super::SystemClock {
        use windows::Win32::Globalization::{
            GetDateFormatEx, GetTimeFormatEx, DATE_SHORTDATE, TIME_NOSECONDS,
        };
        let mut time = [0u16; 64];
        let mut date = [0u16; 64];
        // SAFETY: the user's default locale (null name), now (no time given), local buffers.
        let (t, d) = unsafe {
            (
                GetTimeFormatEx(
                    PCWSTR::null(),
                    TIME_NOSECONDS,
                    None,
                    PCWSTR::null(),
                    Some(&mut time),
                ),
                GetDateFormatEx(
                    PCWSTR::null(),
                    DATE_SHORTDATE,
                    None,
                    PCWSTR::null(),
                    Some(&mut date),
                    PCWSTR::null(),
                ),
            )
        };
        let text = |buf: &[u16], len: i32| {
            String::from_utf16_lossy(&buf[..(len.max(1) as usize - 1).min(buf.len())])
        };
        super::SystemClock {
            time: text(&time, t),
            date: text(&date, d),
        }
    }

    /// A DWORD under the current user's `key`.
    pub fn dword(key: &str, name: &str) -> Option<u32> {
        use windows::{
            core::HSTRING,
            Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
        };
        let mut value = 0u32;
        let mut len = std::mem::size_of::<u32>() as u32;
        // SAFETY: reads one DWORD into a local of the right size.
        let read = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(key),
                &HSTRING::from(name),
                RRF_RT_REG_DWORD,
                None,
                Some((&mut value as *mut u32).cast()),
                Some(&mut len),
            )
        };
        read.is_ok().then_some(value)
    }

    /// A taskbar button found earlier, by its UI Automation name.
    struct Remembered(String, IUIAutomationElement);
    // SAFETY: UI Automation client objects live in the process's multithreaded apartment and
    // may be used from any thread in it; they're only touched from such threads (see `in_mta`).
    unsafe impl Send for Remembered {}

    static REMEMBERED: Mutex<Vec<Remembered>> = Mutex::new(Vec::new());

    /// Run `work` on a fresh thread in the multithreaded COM apartment, and wait for it.
    fn in_mta<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        std::thread::spawn(move || {
            // SAFETY: COM set up for this thread only, torn down after `work`, which drops its
            // own interfaces before returning.
            unsafe {
                let init = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
                let result = work();
                if init {
                    CoUninitialize();
                }
                result
            }
        })
        .join()
        .ok()
    }

    /// The taskbar corner's buttons, while the taskbar is shown (a hidden one shows none).
    fn find_buttons() -> windows::core::Result<Vec<Remembered>> {
        // SAFETY: plain UI Automation calls on interfaces this function owns.
        unsafe {
            let taskbar = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null())?;
            let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            let root = automation.ElementFromHandle(taskbar)?;
            let condition = automation
                .CreatePropertyCondition(UIA_AutomationIdPropertyId, &VARIANT::from("SystemTrayIcon"))?;
            let found = root.FindAll(TreeScope_Descendants, &condition)?;
            let mut buttons = Vec::new();
            for i in 0..found.Length()? {
                let element = found.GetElement(i)?;
                let name = element.CurrentName().map(|n| n.to_string()).unwrap_or_default();
                buttons.push(Remembered(name, element));
            }
            Ok(buttons)
        }
    }

    /// Note the taskbar corner's buttons, to press later while the taskbar is hidden. Call
    /// while the taskbar is shown, just before hiding it; waits as long as UI Automation takes
    /// (well under a second).
    pub fn remember() {
        if let Some(Ok(buttons)) = in_mta(find_buttons) {
            if !buttons.is_empty() {
                *REMEMBERED.lock().unwrap_or_else(|e| e.into_inner()) = buttons;
            }
        }
    }

    fn press_element(element: &IUIAutomationElement) -> bool {
        // SAFETY: plain UI Automation calls on a live interface.
        unsafe {
            element
                .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
                .and_then(|invoke| invoke.Invoke())
                .is_ok()
        }
    }

    /// Press `button` on the taskbar: as found now (the taskbar is shown), or as remembered.
    /// False when neither works (or Windows isn't in English).
    pub fn invoke(button: TrayButton) -> bool {
        in_mta(move || {
            let matches = |name: &str| button.names().iter().any(|prefix| name.starts_with(prefix));
            if let Ok(buttons) = find_buttons() {
                if let Some(found) = buttons.iter().find(|b| matches(&b.0)) {
                    return press_element(&found.1);
                }
            }
            let remembered = REMEMBERED.lock().unwrap_or_else(|e| e.into_inner());
            remembered
                .iter()
                .find(|b| matches(&b.0))
                .is_some_and(|b| press_element(&b.1))
        })
        .unwrap_or(false)
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    ..Default::default()
                },
            },
        }
    }

    /// Press Windows + `letter` (a space for Windows + Space).
    pub fn win_shortcut(letter: char) {
        let vk = if letter == ' ' {
            VK_SPACE
        } else {
            VIRTUAL_KEY(letter.to_ascii_uppercase() as u16)
        };
        let inputs = [
            key(VK_LWIN, false),
            key(vk, false),
            key(vk, true),
            key(VK_LWIN, true),
        ];
        // SAFETY: a local array of well-formed inputs.
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    }

    fn locale_text(name: &[u16], what: u32) -> String {
        let mut buffer = [0u16; 16];
        // SAFETY: `name` is a NUL-terminated locale name; the buffer is local and its size passed.
        let len = unsafe { GetLocaleInfoEx(PCWSTR(name.as_ptr()), what, Some(&mut buffer)) };
        String::from_utf16_lossy(&buffer[..(len.max(1) as usize - 1).min(buffer.len())])
    }

    /// The keyboard language of the window in front, like the taskbar's indicator.
    pub fn input_language() -> InputLanguage {
        // SAFETY: plain queries; the layout list is read into a local buffer of the size given.
        let (layout, count) = unsafe {
            let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
            (GetKeyboardLayout(thread), GetKeyboardLayoutList(None))
        };
        let lang_id = (layout.0 as usize & 0xFFFF) as u32;
        let mut name = [0u16; 85];
        // SAFETY: the buffer is local and its size passed.
        let len = unsafe { LCIDToLocaleName(lang_id, Some(&mut name), 0) };
        if len == 0 {
            return InputLanguage {
                language: String::new(),
                region: String::new(),
                several: count > 1,
            };
        }
        InputLanguage {
            language: locale_text(&name, LOCALE_SISO639LANGNAME2).to_uppercase(),
            region: locale_text(&name, LOCALE_SISO3166CTRYNAME).to_uppercase(),
            several: count > 1,
        }
    }
}
