//! The Windows side of workspaces: which program and arguments each window came from (asked of
//! WMI), where it is, starting programs again and moving their windows back.

use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    time::{Duration, Instant},
};

use windows::{
    core::{w, BSTR, HSTRING, PCWSTR},
    Win32::{
        Foundation::HWND,
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoSetProxyBlanket, CoUninitialize, CLSCTX_INPROC_SERVER,
                COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
            },
            Variant::VARIANT,
            Wmi::{
                IWbemLocator, IWbemServices, WbemLocator, WBEM_FLAG_FORWARD_ONLY,
                WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_GENERIC_FLAG_TYPE, WBEM_INFINITE,
            },
        },
        UI::{
            Shell::{ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW},
            WindowsAndMessaging::{
                GetWindowPlacement, GetWindowThreadProcessId, SetWindowPlacement, SW_SHOWMAXIMIZED,
                SW_SHOWNORMAL, WINDOWPLACEMENT,
            },
        },
    },
};

use super::{args_after_program, WindowPlace, Workspace, WorkspaceApp};
use crate::apps::{Launch, WinInfo};

/// How long a restore waits for started apps to show their windows.
const WAIT_FOR_WINDOWS: Duration = Duration::from_secs(20);

fn wmi() -> Option<IWbemServices> {
    crate::apps::win::com_init();
    // SAFETY: COM calls on objects owned here; COM is initialised on this thread above.
    unsafe {
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).ok()?;
        let services = locator
            .ConnectServer(
                &BSTR::from("ROOT\\CIMV2"),
                &BSTR::new(),
                &BSTR::new(),
                &BSTR::new(),
                0,
                &BSTR::new(),
                None,
            )
            .ok()?;
        // 10 and 0: RPC_C_AUTHN_WINNT and RPC_C_AUTHZ_NONE.
        CoSetProxyBlanket(
            &services,
            10,
            0,
            None,
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )
        .ok()?;
        Some(services)
    }
}

/// Every process's command line, by process id (only processes this user may see).
fn command_lines() -> HashMap<u32, String> {
    let mut lines = HashMap::new();
    let Some(services) = wmi() else { return lines };
    // SAFETY: COM calls on objects owned here; each out-parameter is a local of the right type.
    unsafe {
        let flags = WBEM_GENERIC_FLAG_TYPE(WBEM_FLAG_FORWARD_ONLY.0 | WBEM_FLAG_RETURN_IMMEDIATELY.0);
        let Ok(list) = services.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from("SELECT ProcessId, CommandLine FROM Win32_Process"),
            flags,
            None,
        ) else {
            return lines;
        };
        loop {
            let mut found = [None];
            let mut count = 0;
            if list.Next(WBEM_INFINITE, &mut found, &mut count).is_err() || count == 0 {
                break;
            }
            let Some(object) = found[0].take() else { break };
            let mut pid = VARIANT::default();
            let mut line = VARIANT::default();
            if object.Get(w!("ProcessId"), 0, &mut pid, None, None).is_err()
                || object.Get(w!("CommandLine"), 0, &mut line, None, None).is_err()
            {
                continue;
            }
            let (Ok(pid), Ok(line)) = (u32::try_from(&pid), BSTR::try_from(&line)) else {
                continue;
            };
            lines.insert(pid, line.to_string());
        }
    }
    lines
}

fn pid_of(hwnd: isize) -> u32 {
    let mut pid = 0u32;
    // SAFETY: a plain query on a window handle; a closed window just leaves `pid` at 0.
    unsafe { GetWindowThreadProcessId(HWND(hwnd as *mut c_void), Some(&mut pid)) };
    pid
}

fn place_of(hwnd: isize) -> Option<WindowPlace> {
    let mut placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    // SAFETY: fills a correctly sized local.
    unsafe { GetWindowPlacement(HWND(hwnd as *mut c_void), &mut placement) }.ok()?;
    let r = placement.rcNormalPosition;
    Some(WindowPlace {
        x: r.left,
        y: r.top,
        width: r.right - r.left,
        height: r.bottom - r.top,
        maximized: placement.showCmd == SW_SHOWMAXIMIZED.0 as u32,
    })
}

fn move_to(hwnd: isize, place: WindowPlace) {
    let mut placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    let hwnd = HWND(hwnd as *mut c_void);
    // SAFETY: reads then writes the placement of a window; a closed window makes both fail.
    unsafe {
        if GetWindowPlacement(hwnd, &mut placement).is_err() {
            return;
        }
        placement.rcNormalPosition.left = place.x;
        placement.rcNormalPosition.top = place.y;
        placement.rcNormalPosition.right = place.x + place.width.max(100);
        placement.rcNormalPosition.bottom = place.y + place.height.max(80);
        placement.showCmd = if place.maximized {
            SW_SHOWMAXIMIZED.0 as u32
        } else {
            SW_SHOWNORMAL.0 as u32
        };
        let _ = SetWindowPlacement(hwnd, &placement);
    }
}

/// Packaged apps run from WindowsApps or inside ApplicationFrameHost; they start by app id.
fn is_packaged(exe: &str) -> bool {
    exe.contains(r"\windowsapps\") || exe.ends_with(r"\applicationframehost.exe")
}

/// Windows that aren't worth saving: the dock itself, and Explorer's own windows.
fn skip(window: &WinInfo, own_exe: &str) -> bool {
    match window.exe.as_deref() {
        Some(exe) => exe == own_exe || exe.ends_with(r"\explorer.exe"),
        None => window.app_id.is_none(),
    }
}

/// The apps open now, one entry per window, front-most first.
pub fn capture() -> Vec<WorkspaceApp> {
    let own_exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let windows = crate::apps::win::windows();
    let lines = command_lines();
    windows
        .iter()
        .filter(|w| !skip(w, &own_exe))
        .map(|w| {
            let packaged = w.exe.as_deref().is_none_or(is_packaged) && w.app_id.is_some();
            let args = lines
                .get(&pid_of(w.hwnd))
                .map(|line| args_after_program(line))
                .unwrap_or_default();
            WorkspaceApp {
                name: if w.app_name.is_empty() {
                    w.title.clone()
                } else {
                    w.app_name.clone()
                },
                exe: if packaged { None } else { w.exe.clone() },
                args: if packaged { String::new() } else { args },
                app_id: w.app_id.clone(),
                place: place_of(w.hwnd),
            }
        })
        .collect()
}

/// Start `exe` with `args`, as Explorer would.
fn launch_with_args(exe: &str, args: &str) {
    let (file, params) = (HSTRING::from(exe), HSTRING::from(args));
    let _ = std::thread::Builder::new().name("launch".into()).spawn(move || {
        let info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
            lpVerb: w!("open"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: if params.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(params.as_ptr())
            },
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        let mut info = info;
        // SAFETY: COM is set up and torn down on this thread; `info` and the strings outlive the call.
        unsafe {
            let initialised = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).is_ok();
            if let Err(error) = ShellExecuteExW(&mut info) {
                tracing::warn!(%error, "could not start a workspace app");
            }
            if initialised {
                CoUninitialize();
            }
        }
    });
}

fn start(app: &WorkspaceApp) {
    match (&app.exe, &app.app_id) {
        (Some(exe), _) => launch_with_args(exe, &app.args),
        (None, Some(id)) => crate::apps::win::launch(&Launch::AppId(id.clone())),
        (None, None) => {}
    }
}

/// Put `workspace` back: reuse open windows, start what's missing, then place windows as they
/// appear.
pub fn restore(workspace: &Workspace) {
    let mut claimed: HashSet<isize> = HashSet::new();
    let mut waiting: Vec<&WorkspaceApp> = Vec::new();
    let before = crate::apps::win::windows();
    for saved in &workspace.apps {
        let open = before
            .iter()
            .find(|w| !claimed.contains(&w.hwnd) && saved.matches(w.exe.as_deref(), w.app_id.as_deref()));
        match open {
            Some(window) => {
                claimed.insert(window.hwnd);
                if let Some(place) = saved.place {
                    move_to(window.hwnd, place);
                }
            }
            None => {
                start(saved);
                waiting.push(saved);
            }
        }
    }
    tracing::info!(workspace = %workspace.name, started = waiting.len(), "restoring a workspace");
    let deadline = Instant::now() + WAIT_FOR_WINDOWS;
    while !waiting.is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(500));
        for window in crate::apps::win::windows() {
            if claimed.contains(&window.hwnd) {
                continue;
            }
            let Some(index) = waiting
                .iter()
                .position(|s| s.matches(window.exe.as_deref(), window.app_id.as_deref()))
            else {
                continue;
            };
            claimed.insert(window.hwnd);
            let saved = waiting.remove(index);
            if let Some(place) = saved.place {
                // Apps often place their own window just after showing it; wait that out.
                std::thread::sleep(Duration::from_millis(300));
                move_to(window.hwnd, place);
            }
        }
    }
}
