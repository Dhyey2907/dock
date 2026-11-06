//! USB drives: removable drives and USB disks, with Safely Remove.
//!
//! Ejecting finds the disk behind the drive letter (its device number), the disk's device node
//! (matched through the disk interfaces Windows lists), and asks Plug and Play to eject its
//! parent (the USB device), just as the tray's "Safely Remove Hardware" does.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsbDrive {
    /// "E"
    pub letter: String,
    /// The volume's name, or "USB drive".
    pub label: String,
    #[ts(type = "number")]
    pub total: u64,
    #[ts(type = "number")]
    pub free: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "result", rename_all = "camelCase")]
#[ts(export)]
pub enum EjectResult {
    /// Safe to unplug.
    Ejected,
    /// Something has a file open on it; `by` names it when Windows says.
    InUse {
        #[ts(optional)]
        #[serde(skip_serializing_if = "Option::is_none")]
        by: Option<String>,
    },
    /// The drive is gone, or isn't one that can be ejected.
    Failed,
}

/// Drive letters present in `GetLogicalDrives`' bit mask (bit 0 = A).
pub fn drive_letters(mask: u32) -> Vec<char> {
    (0..26u8)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| (b'A' + bit) as char)
        .collect()
}

/// A device instance id from a device interface path:
/// `\\?\USBSTOR#Disk&Ven_X#0123&0#{guid}` → `USBSTOR\Disk&Ven_X\0123&0`.
pub fn instance_id_from_interface(path: &str) -> Option<String> {
    let rest = path.strip_prefix(r"\\?\").unwrap_or(path);
    let without_guid = match rest.rfind("#{") {
        Some(at) => &rest[..at],
        None => rest,
    };
    let parts: Vec<&str> = without_guid.split('#').collect();
    (parts.len() >= 3 && parts.iter().all(|p| !p.is_empty())).then(|| parts.join("\\"))
}

/// Whether a disk interface belongs to a USB disk (a USB hard drive shows up as a fixed drive,
/// so the drive type alone misses it).
pub fn is_usb_interface(path: &str) -> bool {
    let upper = path.to_ascii_uppercase();
    upper.contains("USBSTOR") || upper.contains("UASPSTOR") || upper.contains("#USB")
}

#[cfg(windows)]
mod win {
    use windows::{
        core::PCWSTR,
        Win32::{
            Devices::DeviceAndDriverInstallation::{
                CM_Get_Device_Interface_ListW, CM_Get_Device_Interface_List_SizeW, CM_Get_Parent,
                CM_Locate_DevNodeW, CM_Request_Device_EjectW, CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
                CM_LOCATE_DEVNODE_NORMAL, CR_SUCCESS, PNP_VETO_TYPE,
            },
            Foundation::{CloseHandle, HANDLE},
            Storage::FileSystem::{
                CreateFileW, GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
                FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            },
            System::{
                Ioctl::{GUID_DEVINTERFACE_DISK, IOCTL_STORAGE_GET_DEVICE_NUMBER, STORAGE_DEVICE_NUMBER},
                IO::DeviceIoControl,
            },
        },
    };

    use super::{drive_letters, instance_id_from_interface, is_usb_interface, EjectResult, UsbDrive};
    use crate::system::{to_wide, wide};

    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;

    /// The disk number behind a device path (`\\.\E:` or a disk interface path).
    fn device_number(path: &str) -> Option<u32> {
        let wide_path = to_wide(path);
        // SAFETY: opened with no access rights (enough for this query) and closed after.
        unsafe {
            let handle: HANDLE = CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
            .ok()?;
            let mut number = STORAGE_DEVICE_NUMBER::default();
            let mut returned = 0u32;
            let ok = DeviceIoControl(
                handle,
                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                None,
                0,
                Some(&mut number as *mut STORAGE_DEVICE_NUMBER as *mut core::ffi::c_void),
                std::mem::size_of::<STORAGE_DEVICE_NUMBER>() as u32,
                Some(&mut returned),
                None,
            )
            .is_ok();
            let _ = CloseHandle(handle);
            ok.then_some(number.DeviceNumber)
        }
    }

    /// Every disk interface path Windows has right now.
    fn disk_interfaces() -> Vec<String> {
        // SAFETY: the buffer is sized by the first call; the list is NUL-separated, NUL-ended.
        unsafe {
            let mut size = 0u32;
            if CM_Get_Device_Interface_List_SizeW(
                &mut size,
                &GUID_DEVINTERFACE_DISK,
                PCWSTR::null(),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            ) != CR_SUCCESS
            {
                return Vec::new();
            }
            let mut buffer = vec![0u16; size as usize + 1];
            if CM_Get_Device_Interface_ListW(
                &GUID_DEVINTERFACE_DISK,
                PCWSTR::null(),
                &mut buffer,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            ) != CR_SUCCESS
            {
                return Vec::new();
            }
            buffer
                .split(|&c| c == 0)
                .filter(|s| !s.is_empty())
                .map(String::from_utf16_lossy)
                .collect()
        }
    }

    /// The disk interface for a drive letter.
    fn interface_for(letter: char, interfaces: &[String]) -> Option<String> {
        let number = device_number(&format!(r"\\.\{letter}:"))?;
        interfaces
            .iter()
            .find(|path| device_number(path) == Some(number))
            .cloned()
    }

    fn system_drive() -> Option<char> {
        std::env::var("SystemDrive")
            .ok()?
            .chars()
            .next()
            .map(|c| c.to_ascii_uppercase())
    }

    pub fn list() -> Vec<UsbDrive> {
        let interfaces = disk_interfaces();
        let system = system_drive();
        // SAFETY: plain queries on root paths.
        let mask = unsafe { GetLogicalDrives() };
        let mut drives = Vec::new();
        for letter in drive_letters(mask) {
            if Some(letter) == system {
                continue;
            }
            let root = to_wide(&format!(r"{letter}:\"));
            let kind = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
            let usb = match kind {
                DRIVE_REMOVABLE => true,
                DRIVE_FIXED => interface_for(letter, &interfaces).is_some_and(|p| is_usb_interface(&p)),
                _ => false,
            };
            if !usb {
                continue;
            }
            let mut label = [0u16; 261];
            let (mut free, mut total) = (0u64, 0u64);
            // SAFETY: buffers outlive the calls; a card reader with no card fails both.
            unsafe {
                let _ =
                    GetVolumeInformationW(PCWSTR(root.as_ptr()), Some(&mut label), None, None, None, None);
                let _ = GetDiskFreeSpaceExW(PCWSTR(root.as_ptr()), Some(&mut free), Some(&mut total), None);
            }
            let name = wide(&label);
            drives.push(UsbDrive {
                letter: letter.to_string(),
                label: if name.trim().is_empty() {
                    "USB drive".into()
                } else {
                    name
                },
                total,
                free,
            });
        }
        drives
    }

    /// Ask Plug and Play to eject `devinst`.
    fn request_eject(devinst: u32) -> EjectResult {
        let mut veto = PNP_VETO_TYPE::default();
        let mut name = [0u16; 260];
        // SAFETY: out-buffers sized as passed.
        let result = unsafe { CM_Request_Device_EjectW(devinst, Some(&mut veto), Some(&mut name), 0) };
        if result == CR_SUCCESS && veto.0 == 0 {
            EjectResult::Ejected
        } else if veto.0 != 0 {
            let by = wide(&name);
            EjectResult::InUse {
                by: (!by.trim().is_empty()).then_some(by),
            }
        } else {
            EjectResult::Failed
        }
    }

    pub fn eject(letter: char) -> EjectResult {
        let interfaces = disk_interfaces();
        let Some(id) = interface_for(letter, &interfaces).and_then(|p| instance_id_from_interface(&p)) else {
            return EjectResult::Failed;
        };
        let wide_id = to_wide(&id);
        let mut disk = 0u32;
        // SAFETY: locating a present device node by its instance id.
        let located =
            unsafe { CM_Locate_DevNodeW(&mut disk, PCWSTR(wide_id.as_ptr()), CM_LOCATE_DEVNODE_NORMAL) };
        if located != CR_SUCCESS {
            return EjectResult::Failed;
        }
        // The USB device above the disk is what's ejected; the disk itself if it has no parent
        // that can be.
        let mut parent = 0u32;
        // SAFETY: walking up from a node located above.
        if unsafe { CM_Get_Parent(&mut parent, disk, 0) } == CR_SUCCESS {
            match request_eject(parent) {
                EjectResult::Failed => {}
                done => return done,
            }
        }
        request_eject(disk)
    }
}

#[cfg(windows)]
pub fn list() -> Vec<UsbDrive> {
    win::list()
}

#[cfg(not(windows))]
pub fn list() -> Vec<UsbDrive> {
    Vec::new()
}

/// Safely remove the drive at `letter`.
pub fn eject(letter: &str) -> crate::error::AppResult<EjectResult> {
    let mut chars = letter.trim().chars();
    let (Some(first), None) = (chars.next(), chars.next()) else {
        return Err(crate::error::AppError::InvalidInput("drive letter".into()));
    };
    if !first.is_ascii_alphabetic() {
        return Err(crate::error::AppError::InvalidInput("drive letter".into()));
    }
    // Only drives this list would show: never the system drive or an internal disk.
    let letter = first.to_ascii_uppercase();
    if !list().iter().any(|d| d.letter.starts_with(letter)) {
        return Ok(EjectResult::Failed);
    }
    #[cfg(windows)]
    return Ok(win::eject(letter));
    #[cfg(not(windows))]
    Ok(EjectResult::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_drive_letters_from_the_mask() {
        assert_eq!(drive_letters(0b1_0100), vec!['C', 'E']);
        assert_eq!(drive_letters(0), Vec::<char>::new());
        assert_eq!(drive_letters(1 << 25), vec!['Z']);
    }

    #[test]
    fn interface_paths_become_instance_ids() {
        assert_eq!(
            instance_id_from_interface(
                r"\\?\USBSTOR#Disk&Ven_SanDisk&Prod_Ultra&Rev_1.00#4C530001#{53f56307-b6bf-11d0-94f2-00a0c91efb8b}"
            )
            .as_deref(),
            Some(r"USBSTOR\Disk&Ven_SanDisk&Prod_Ultra&Rev_1.00\4C530001")
        );
        assert_eq!(instance_id_from_interface(r"\\?\nonsense"), None);
    }

    #[test]
    fn spots_usb_disks() {
        assert!(is_usb_interface(r"\\?\usbstor#disk&ven_wd#123#{guid}"));
        assert!(is_usb_interface(
            r"\\?\SCSI#Disk&Ven_Samsung&Prod_T7#UASPSTOR&1#{guid}"
        ));
        assert!(!is_usb_interface(
            r"\\?\SCSI#Disk&Ven_NVMe&Prod_Samsung#5&1#{guid}"
        ));
    }

    #[test]
    fn bad_letters_are_refused() {
        assert!(eject("").is_err());
        assert!(eject("EF").is_err());
        assert!(eject("1").is_err());
    }
}
