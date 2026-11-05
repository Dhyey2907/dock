//! The Windows side of the apps list: pins, windows, icons, and acting on them.

use std::{
    collections::HashMap,
    ffi::c_void,
    path::PathBuf,
    sync::{Mutex, Once},
};

use windows::{
    core::{w, Interface, BOOL, HSTRING, PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, PROPERTYKEY, SIZE, WPARAM},
        Graphics::{
            Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED},
            Gdi::{
                DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
                BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
            },
        },
        Storage::{
            FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
            Packaging::Appx::GetApplicationUserModelId,
        },
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
                COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, COINIT_MULTITHREADED, STGM_READ,
            },
            Registry::{
                RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_NOTIFY,
                KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, RRF_RT_REG_BINARY,
            },
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::{
            Accessibility::{SetWinEventHook, HWINEVENTHOOK},
            Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
                VIRTUAL_KEY, VK_MENU,
            },
            Shell::{
                IShellItem, IShellItemImageFactory, IShellLinkW,
                PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow},
                SHCreateItemFromParsingName, ShellExecuteExW, ShellLink, SEE_MASK_FLAG_NO_UI,
                SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, SIGDN_NORMALDISPLAY, SIIGBF_ICONONLY,
            },
            WindowsAndMessaging::{
                EnumWindows, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowTextLengthW,
                GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW,
                SetForegroundWindow, ShowWindowAsync, CHILDID_SELF, EVENT_OBJECT_CLOAKED,
                EVENT_OBJECT_CREATE, EVENT_OBJECT_HIDE, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_UNCLOAKED,
                EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART, GWL_EXSTYLE,
                GW_OWNER, OBJID_WINDOW, SW_MINIMIZE, SW_RESTORE, SW_SHOWNORMAL, WINEVENT_OUTOFCONTEXT,
                WINEVENT_SKIPOWNPROCESS, WM_CLOSE,
            },
        },
    },
};

use super::{parse_taskband, shows_on_taskbar, Launch, Pin, PinRef, WinInfo, WindowTraits};

const TASKBAND_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Taskband";
/// System.AppUserModel.ID
const APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: windows::core::GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3),
    pid: 5,
};

/// Shell objects need COM on the calling thread; repeat calls are harmless. Multithreaded,
/// because these run on worker threads that never pump messages (a single-threaded apartment
/// there could hang anything that calls back into it).
pub fn com_init() {
    // SAFETY: initialising COM for this thread; an already-initialised thread just gets S_FALSE
    // (or RPC_E_CHANGED_MODE when it chose another model, which still leaves COM usable).
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
}

fn pinned_folder() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar"))
}

fn taskband_blob() -> Vec<u8> {
    let key = HSTRING::from(TASKBAND_KEY);
    let mut len = 0u32;
    // SAFETY: the first call asks only for the size; the second fills a buffer of that size.
    unsafe {
        let sized = RegGetValueW(
            HKEY_CURRENT_USER,
            &key,
            w!("Favorites"),
            RRF_RT_REG_BINARY,
            None,
            None,
            Some(&mut len),
        );
        if sized.is_err() || len == 0 {
            return Vec::new();
        }
        let mut blob = vec![0u8; len as usize];
        let read = RegGetValueW(
            HKEY_CURRENT_USER,
            &key,
            w!("Favorites"),
            RRF_RT_REG_BINARY,
            None,
            Some(blob.as_mut_ptr().cast()),
            Some(&mut len),
        );
        if read.is_err() {
            return Vec::new();
        }
        blob.truncate(len as usize);
        blob
    }
}

/// The taskbar's pinned apps, in its order.
pub fn read_pins() -> Vec<Pin> {
    com_init();
    let Some(folder) = pinned_folder() else {
        return Vec::new();
    };
    let shortcuts: Vec<String> = std::fs::read_dir(&folder)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| name.to_lowercase().ends_with(".lnk"))
                .collect()
        })
        .unwrap_or_default();
    parse_taskband(&taskband_blob(), &shortcuts)
        .into_iter()
        .filter_map(|pin| match pin {
            PinRef::Shortcut(file) => {
                let path = folder.join(&file).to_string_lossy().into_owned();
                let (exe, app_id) = resolve_shortcut(&path);
                Some(Pin {
                    name: file.trim_end_matches(".lnk").trim_end_matches(".LNK").to_string(),
                    app_id,
                    exe,
                    icon: format!("lnk:{path}"),
                    launch: Launch::Shortcut(path),
                })
            }
            PinRef::AppId(id) => Some(Pin {
                // An app id that no longer resolves to an installed app is a stale pin.
                name: app_id_name(&id)?,
                icon: format!("aumid:{id}"),
                launch: Launch::AppId(id.clone()),
                app_id: Some(id),
                exe: None,
            }),
        })
        .collect()
}

/// A shortcut's program (lowercased) and app id.
fn resolve_shortcut(path: &str) -> (Option<String>, Option<String>) {
    // SAFETY: COM calls on objects owned by this function; buffers are locals of the given size.
    unsafe {
        let Ok(link) = CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) else {
            return (None, None);
        };
        let Ok(file) = link.cast::<IPersistFile>() else {
            return (None, None);
        };
        if file.Load(&HSTRING::from(path), STGM_READ).is_err() {
            return (None, None);
        }
        let mut target = [0u16; 1024];
        let exe = link
            .GetPath(&mut target, std::ptr::null_mut(), 0)
            .ok()
            .map(|()| wide_to_string(&target).to_lowercase())
            .filter(|t| t.ends_with(".exe"));
        let app_id = link
            .cast::<IPropertyStore>()
            .ok()
            .and_then(|store| store.GetValue(&APP_USER_MODEL_ID).ok())
            .map(|value| value.to_string().trim().to_string())
            .filter(|id| !id.is_empty());
        (exe, app_id)
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// An installed app's name, by app id, as the Start menu shows it.
fn app_id_name(id: &str) -> Option<String> {
    // SAFETY: COM calls on a shell item owned by this function; the name buffer is freed by
    // `PWSTR::to_string` + CoTaskMemFree inside the `windows` wrapper.
    unsafe {
        let item: IShellItem =
            SHCreateItemFromParsingName(&HSTRING::from(format!("shell:AppsFolder\\{id}")), None).ok()?;
        let name = item.GetDisplayName(SIGDN_NORMALDISPLAY).ok()?;
        let text = name.to_string().ok();
        windows::Win32::System::Com::CoTaskMemFree(Some(name.0 as *const c_void));
        text.filter(|t| !t.is_empty())
    }
}

/// A program's description ("Google Chrome"), falling back to its file name.
fn program_name(exe: &str) -> String {
    let fallback = || {
        std::path::Path::new(exe)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| exe.to_string())
    };
    let path = HSTRING::from(exe);
    // SAFETY: the version block is sized by the first call and read with VerQueryValueW, whose
    // out-pointers point into that block while it is alive.
    let description = unsafe {
        let size = GetFileVersionInfoSizeW(&path, None);
        if size == 0 {
            return fallback();
        }
        let mut block = vec![0u8; size as usize];
        if GetFileVersionInfoW(&path, None, size, block.as_mut_ptr().cast()).is_err() {
            return fallback();
        }
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let lang = if VerQueryValueW(
            block.as_ptr().cast(),
            w!("\\VarFileInfo\\Translation"),
            &mut ptr,
            &mut len,
        )
        .as_bool()
            && len >= 4
        {
            let pair = std::slice::from_raw_parts(ptr as *const u16, 2);
            format!("{:04x}{:04x}", pair[0], pair[1])
        } else {
            "040904b0".to_string()
        };
        let query = HSTRING::from(format!("\\StringFileInfo\\{lang}\\FileDescription"));
        if VerQueryValueW(block.as_ptr().cast(), &query, &mut ptr, &mut len).as_bool() && len > 0 {
            let text = std::slice::from_raw_parts(ptr as *const u16, len as usize);
            Some(wide_to_string(text).trim().to_string())
        } else {
            None
        }
    };
    description.filter(|d| !d.is_empty()).unwrap_or_else(fallback)
}

fn process_exe(pid: u32) -> Option<String> {
    // SAFETY: a limited query handle, closed before returning; the buffer is a local.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        ok.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]).to_lowercase())
    }
}

/// A packaged app's id, from its process.
fn process_app_id(pid: u32) -> Option<String> {
    // SAFETY: a limited query handle, closed before returning; the buffer is a local.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 256];
        let mut len = buf.len() as u32;
        let status = GetApplicationUserModelId(process, &mut len, Some(PWSTR(buf.as_mut_ptr())));
        let _ = CloseHandle(process);
        (status.is_ok()).then(|| wide_to_string(&buf))
    }
}

/// The window's own app id (set by apps like Chrome profiles), else its package's.
fn window_app_id(hwnd: HWND, pid: u32) -> Option<String> {
    // SAFETY: COM calls on a live window's property store, released when dropped.
    let explicit = unsafe {
        SHGetPropertyStoreForWindow::<IPropertyStore>(hwnd)
            .ok()
            .and_then(|store| store.GetValue(&APP_USER_MODEL_ID).ok())
            .map(|value| value.to_string().trim().to_string())
    };
    explicit
        .filter(|id| !id.is_empty())
        .or_else(|| process_app_id(pid))
}

/// Friendly names, cached by app id or program: they don't change while the dock runs.
static NAMES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

fn cached_name(key: &str, compute: impl FnOnce() -> String) -> String {
    if let Some(name) = NAMES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(key))
    {
        return name.clone();
    }
    let name = compute();
    NAMES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(key.to_string(), name.clone());
    name
}

/// Every window that would get a taskbar button, front-most first.
pub fn windows() -> Vec<WinInfo> {
    com_init();
    unsafe extern "system" fn collect(hwnd: HWND, list: LPARAM) -> BOOL {
        // SAFETY: `list` is the Vec passed to EnumWindows below, alive for the whole call.
        unsafe { (*(list.0 as *mut Vec<HWND>)).push(hwnd) };
        true.into()
    }
    let mut handles: Vec<HWND> = Vec::new();
    // SAFETY: the callback only pushes into `handles`, which outlives the call.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&mut handles as *mut _ as isize)) };
    let own_pid = std::process::id();
    handles
        .into_iter()
        .filter_map(|hwnd| {
            // SAFETY: read-only queries on a window handle; a window closing meanwhile just
            // makes them fail.
            let (pid, traits, minimized) = unsafe {
                let mut pid = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                let mut cloaked = 0u32;
                let _ = DwmGetWindowAttribute(
                    hwnd,
                    DWMWA_CLOAKED,
                    (&mut cloaked as *mut u32).cast(),
                    std::mem::size_of::<u32>() as u32,
                );
                let traits = WindowTraits {
                    visible: IsWindowVisible(hwnd).as_bool(),
                    has_owner: GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid()),
                    ex_style: GetWindowLongW(hwnd, GWL_EXSTYLE) as u32,
                    cloaked: cloaked != 0,
                    has_title: GetWindowTextLengthW(hwnd) > 0,
                    ours: pid == own_pid,
                };
                (pid, traits, IsIconic(hwnd).as_bool())
            };
            if !shows_on_taskbar(traits) {
                return None;
            }
            let mut title = [0u16; 512];
            // SAFETY: fills a local buffer of the given length.
            let len = unsafe { GetWindowTextW(hwnd, &mut title) } as usize;
            let exe = process_exe(pid);
            let app_id = window_app_id(hwnd, pid).or_else(|| {
                exe.as_deref()
                    .and_then(super::implicit_app_id)
                    .map(str::to_string)
            });
            let app_name = match (&app_id, &exe) {
                (Some(id), _) => cached_name(id, || {
                    app_id_name(id)
                        .unwrap_or_else(|| exe.as_deref().map(program_name).unwrap_or_else(|| id.clone()))
                }),
                (None, Some(exe)) => cached_name(exe, || program_name(exe)),
                (None, None) => String::from_utf16_lossy(&title[..len]),
            };
            Some(WinInfo {
                hwnd: hwnd.0 as isize,
                title: String::from_utf16_lossy(&title[..len]),
                exe,
                app_id,
                minimized,
                app_name,
            })
        })
        .collect()
}

pub fn foreground() -> isize {
    // SAFETY: a plain query.
    unsafe { GetForegroundWindow() }.0 as isize
}

// ---------- Icons ----------

/// The icon for `source` ("lnk:<path>", "exe:<path>" or "aumid:<app id>") as a `size`-pixel PNG.
pub fn icon_png(source: &str, size: i32) -> Option<Vec<u8>> {
    com_init();
    let parsing = match source.split_once(':')? {
        ("lnk" | "exe", path) => path.to_string(),
        ("aumid", id) => format!("shell:AppsFolder\\{id}"),
        _ => return None,
    };
    // SAFETY: COM calls on objects owned by this function; the bitmap is read into a local
    // buffer sized from its own header and deleted before returning.
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(parsing), None).ok()?;
        let factory: IShellItemImageFactory = item.cast().ok()?;
        let bitmap = factory
            .GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY)
            .ok()?;
        let pixels = bitmap_rgba(bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let (width, height, rgba) = pixels?;
        encode_png(width, height, &rgba)
    }
}

/// A 32-bit bitmap's pixels as straight (not premultiplied) RGBA.
unsafe fn bitmap_rgba(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut header = BITMAP::default();
    // SAFETY: (caller) `bitmap` is a live GDI bitmap; `header` is a correctly sized local.
    let got = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut header as *mut BITMAP).cast()),
        )
    };
    let (width, height) = (header.bmWidth, header.bmHeight);
    if got == 0 || width <= 0 || height <= 0 {
        return None;
    }
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; width as usize * height as usize * 4];
    // SAFETY: the buffer holds exactly `height` rows of `width` 32-bit pixels.
    let rows = unsafe {
        let dc = GetDC(None);
        let rows = GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(bgra.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        rows
    };
    if rows == 0 {
        return None;
    }
    Some((width as u32, height as u32, straight_rgba(bgra)))
}

/// BGRA (premultiplied, or with no alpha at all) to straight RGBA.
pub fn straight_rgba(mut pixels: Vec<u8>) -> Vec<u8> {
    let has_alpha = pixels.chunks_exact(4).any(|p| p[3] != 0);
    for p in pixels.chunks_exact_mut(4) {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = 255;
        } else if p[3] > 0 && p[3] < 255 {
            let a = u32::from(p[3]);
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    pixels
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().ok()?.write_image_data(rgba).ok()?;
    Some(out)
}

// ---------- Acting on apps ----------

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

/// Press `keys` together (down in order, up in reverse), e.g. Win, or Win+S. The shell ignores
/// a Windows key whose press and release arrive in the same instant, so they are sent apart.
pub fn press(keys: &[VIRTUAL_KEY]) {
    let downs: Vec<INPUT> = keys.iter().map(|&k| key(k, false)).collect();
    let ups: Vec<INPUT> = keys.iter().rev().map(|&k| key(k, true)).collect();
    // SAFETY: sends fully initialised sequences of key events.
    unsafe {
        SendInput(&downs, std::mem::size_of::<INPUT>() as i32);
        std::thread::sleep(std::time::Duration::from_millis(40));
        SendInput(&ups, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Bring a window to the front, restoring it if minimized. Async window calls only: the plain
/// ones wait for every window involved (including the one losing the front), and a hung or
/// suspended app would hang the dock with it.
pub fn activate(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut c_void);
    // SAFETY: plain window calls; a window that closed meanwhile just makes them fail.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
        }
        if !SetForegroundWindow(hwnd).as_bool() {
            // Windows only lets the app that got the last input change the foreground; a tap
            // of Alt counts as that input.
            press(&[VK_MENU]);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

pub fn minimize(hwnd: isize) {
    // SAFETY: a plain window call that doesn't wait for the window.
    let _ = unsafe { ShowWindowAsync(HWND(hwnd as *mut c_void), SW_MINIMIZE) };
}

/// Ask a window to close, as its own close button would.
pub fn close(hwnd: isize) {
    // SAFETY: posting a standard message; the window decides what to do with it.
    let _ = unsafe { PostMessageW(Some(HWND(hwnd as *mut c_void)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
}

/// Start an app the way Explorer would. Runs on its own short-lived thread with the
/// single-threaded COM that ShellExecute wants; `SEE_MASK_NOASYNC` makes it finish before the
/// thread exits.
pub fn launch(launch: &Launch) {
    let target = match launch {
        Launch::Shortcut(path) | Launch::Program(path) => path.clone(),
        Launch::AppId(id) => format!("shell:AppsFolder\\{id}"),
    };
    let _ = std::thread::Builder::new().name("launch".into()).spawn(move || {
        let file = HSTRING::from(target);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
            lpVerb: w!("open"),
            lpFile: PCWSTR(file.as_ptr()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        // SAFETY: COM is set up and torn down on this thread; `info` and `file` outlive the call.
        unsafe {
            let initialised = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).is_ok();
            if let Err(error) = ShellExecuteExW(&mut info) {
                tracing::warn!(%error, "could not start an app");
            }
            if initialised {
                CoUninitialize();
            }
        }
    });
}

// ---------- Watching ----------

/// Call `changed` when the taskbar's pins change (added, removed or reordered).
pub fn watch_pins(changed: impl Fn() + Send + 'static) {
    let _ = std::thread::Builder::new().name("pins".into()).spawn(move || {
        let mut hkey = HKEY::default();
        // SAFETY: opens a key for notifications only; it stays open for the app's life.
        let opened = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(TASKBAND_KEY),
                None,
                KEY_NOTIFY | KEY_READ,
                &mut hkey,
            )
        };
        if opened.is_err() {
            tracing::warn!("could not watch the taskbar's pins");
            return;
        }
        loop {
            // SAFETY: a synchronous wait on our own open key.
            let waited =
                unsafe { RegNotifyChangeKeyValue(hkey, false, REG_NOTIFY_CHANGE_LAST_SET, None, false) };
            if waited.is_err() {
                return;
            }
            changed();
        }
    });
}

static WINDOW_CHANGED: Mutex<Option<Box<dyn Fn() + Send>>> = Mutex::new(None);

/// Call `changed` when windows open, close, retitle, minimize, restore, hide, move to another
/// desktop, or come to the front. Install on the main thread, whose message loop delivers the
/// events; callers should coalesce, since these come in bursts.
pub fn watch_windows(changed: impl Fn() + Send + 'static) {
    static INSTALL: Once = Once::new();
    *WINDOW_CHANGED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(changed));

    unsafe extern "system" fn on_event(
        _hook: HWINEVENTHOOK,
        event: u32,
        _hwnd: HWND,
        id_object: i32,
        id_child: i32,
        _thread: u32,
        _time: u32,
    ) {
        // Events also fire for controls, carets and menus; only whole windows matter.
        let whole_window = id_object == OBJID_WINDOW.0 && id_child == CHILDID_SELF as i32;
        if whole_window || event == EVENT_SYSTEM_FOREGROUND {
            if let Some(changed) = WINDOW_CHANGED.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                changed();
            }
        }
    }

    INSTALL.call_once(|| {
        let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        let ranges = [
            (EVENT_OBJECT_CREATE, EVENT_OBJECT_HIDE), // create, destroy, show, hide
            (EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
            (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
            (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
            (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
        ];
        for (first, last) in ranges {
            // SAFETY: an out-of-context hook with a `'static` callback, installed on the UI
            // thread for the life of the app; the callback only reads a static.
            let hook = unsafe { SetWinEventHook(first, last, None, Some(on_event), 0, 0, flags) };
            if hook.is_invalid() {
                tracing::warn!(first, "could not watch windows for the dock");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::straight_rgba;

    #[test]
    fn premultiplied_bgra_becomes_straight_rgba() {
        // Half-transparent pure red, premultiplied: B=0 G=0 R=128 A=128.
        assert_eq!(straight_rgba(vec![0, 0, 128, 128]), vec![255, 0, 0, 128]);
        // Opaque and fully transparent pixels are left alone (apart from the channel swap).
        assert_eq!(
            straight_rgba(vec![1, 2, 3, 255, 0, 0, 0, 0]),
            vec![3, 2, 1, 255, 0, 0, 0, 0]
        );
    }

    #[test]
    fn bitmaps_without_alpha_become_opaque() {
        assert_eq!(straight_rgba(vec![10, 20, 30, 0]), vec![30, 20, 10, 255]);
    }
}
