//! Bluetooth devices you've paired, whether they're connected, and their battery levels.
//!
//! Devices are read as Windows' association endpoints (both classic and Low Energy). Batteries
//! come from the battery level Windows itself shows in Settings (a property on the device's
//! nodes, found through the device's container), and for Low Energy devices that don't report
//! one, from the standard GATT Battery Service.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use ts_rs::TS;

use crate::automations::{Bubble, BubbleKind};

/// Batteries at or below this warn once.
pub const LOW_BATTERY: u8 = 20;
/// A device that has charged back above this may warn again.
pub const RECHARGED: u8 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BluetoothDevice {
    pub id: String,
    pub name: String,
    pub connected: bool,
    /// Percent, when the device reports it.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery: Option<u8>,
}

/// One endpoint as Windows lists it; a dual-mode device has two (classic and LE) sharing a
/// container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub id: String,
    pub name: String,
    pub container: Option<String>,
    pub connected: bool,
    pub battery: Option<u8>,
}

/// One row per device: endpoints sharing a container merged (connected if either is, the
/// battery whichever reports it). Connected devices first, then by name.
pub fn merge(endpoints: Vec<Endpoint>) -> Vec<BluetoothDevice> {
    let mut devices: Vec<(Option<String>, BluetoothDevice)> = Vec::new();
    for endpoint in endpoints {
        if endpoint.name.trim().is_empty() {
            continue;
        }
        let same = devices
            .iter_mut()
            .find(|(container, _)| container.is_some() && *container == endpoint.container);
        match same {
            Some((_, device)) => {
                device.connected |= endpoint.connected;
                device.battery = device.battery.or(endpoint.battery);
            }
            None => devices.push((
                endpoint.container,
                BluetoothDevice {
                    id: endpoint.id,
                    name: endpoint.name.trim().to_string(),
                    connected: endpoint.connected,
                    battery: endpoint.battery.map(|b| b.min(100)),
                },
            )),
        }
    }
    let mut devices: Vec<BluetoothDevice> = devices.into_iter().map(|(_, d)| d).collect();
    devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    devices
}

/// Which devices have been warned about, so each low battery warns once per discharge.
#[derive(Debug, Default)]
pub struct BatteryMemory {
    warned: HashSet<String>,
}

/// Connected devices whose battery just fell to the low mark.
pub fn newly_low<'a>(devices: &'a [BluetoothDevice], memory: &mut BatteryMemory) -> Vec<&'a BluetoothDevice> {
    let mut low = Vec::new();
    for device in devices {
        let Some(level) = device.battery.filter(|_| device.connected) else {
            continue;
        };
        if level > RECHARGED {
            memory.warned.remove(&device.id);
        } else if level <= LOW_BATTERY && memory.warned.insert(device.id.clone()) {
            low.push(device);
        }
    }
    low
}

pub fn low_battery_bubble(device: &BluetoothDevice, now: DateTime<Utc>) -> Bubble {
    Bubble {
        id: format!("bluetooth:{}", device.id),
        kind: BubbleKind::Bluetooth,
        title: format!("{} battery is low", device.name),
        body: format!("{}% left · charge it soon", device.battery.unwrap_or(0)),
        expires_at: now + Duration::seconds(30),
    }
}

#[cfg(windows)]
mod win {
    use windows::{
        core::{Interface, GUID, HSTRING},
        Devices::{
            Bluetooth::{BluetoothCacheMode, BluetoothLEDevice},
            Enumeration::{DeviceInformation, DeviceInformationKind},
        },
        Foundation::IReference,
        Storage::Streams::DataReader,
    };

    use windows_collections::IIterable;

    use super::Endpoint;

    const CLASSIC: &str = "{e0cbf06c-cd8b-4647-bb8a-263b43f0f974}";
    const LOW_ENERGY: &str = "{bb7bb05e-5972-42b5-94fc-76eaa7084d49}";
    const IS_CONNECTED: &str = "System.Devices.Aep.IsConnected";
    const CONTAINER: &str = "System.Devices.Aep.ContainerId";
    const PROTOCOL: &str = "System.Devices.Aep.ProtocolId";
    /// The battery level Windows shows in Bluetooth settings (DEVPKEY_Bluetooth_Battery).
    const BATTERY: &str = "{104EA319-6EE2-4701-BD47-8DDBF425BBE5} 2";
    const BATTERY_SERVICE: u128 = 0x0000180F_0000_1000_8000_00805F9B34FB;
    const BATTERY_LEVEL: u128 = 0x00002A19_0000_1000_8000_00805F9B34FB;

    fn properties(names: &[&str]) -> IIterable<HSTRING> {
        IIterable::<HSTRING>::from(names.iter().map(|n| HSTRING::from(*n)).collect::<Vec<_>>())
    }

    fn property<T: windows::core::RuntimeType + 'static>(info: &DeviceInformation, name: &str) -> Option<T>
    where
        IReference<T>: Interface,
    {
        let value = info.Properties().ok()?.Lookup(&HSTRING::from(name)).ok()?;
        value.cast::<IReference<T>>().ok()?.Value().ok()
    }

    /// The battery Windows shows for the device in `container`.
    fn container_battery(container: &GUID) -> Option<u8> {
        let aqs = HSTRING::from(format!("System.Devices.ContainerId:=\"{{{container:?}}}\""));
        let nodes = DeviceInformation::FindAllAsyncWithKindAqsFilterAndAdditionalProperties(
            &aqs,
            &properties(&[BATTERY]),
            DeviceInformationKind::Device,
        )
        .ok()?
        .get()
        .ok()?;
        (0..nodes.Size().ok()?)
            .filter_map(|i| nodes.GetAt(i).ok())
            .find_map(|node| property::<u8>(&node, BATTERY))
    }

    /// The level from the GATT Battery Service, for a connected Low Energy device.
    fn gatt_battery(id: &HSTRING) -> Option<u8> {
        let device = BluetoothLEDevice::FromIdAsync(id).ok()?.get().ok()?;
        let services = device
            .GetGattServicesForUuidWithCacheModeAsync(
                GUID::from_u128(BATTERY_SERVICE),
                BluetoothCacheMode::Cached,
            )
            .ok()?
            .get()
            .ok()?
            .Services()
            .ok()?;
        let service = services.GetAt(0).ok()?;
        let characteristic = service
            .GetCharacteristicsForUuidAsync(GUID::from_u128(BATTERY_LEVEL))
            .ok()?
            .get()
            .ok()?
            .Characteristics()
            .ok()?
            .GetAt(0)
            .ok()?;
        let buffer = characteristic
            .ReadValueWithCacheModeAsync(BluetoothCacheMode::Uncached)
            .ok()?
            .get()
            .ok()?
            .Value()
            .ok()?;
        let reader = DataReader::FromBuffer(&buffer).ok()?;
        reader.ReadByte().ok()
    }

    pub fn endpoints() -> Vec<Endpoint> {
        let aqs = HSTRING::from(format!(
            "({PROTOCOL}:=\"{CLASSIC}\" OR {PROTOCOL}:=\"{LOW_ENERGY}\") AND \
             System.Devices.Aep.IsPaired:=System.StructuredQueryType.Boolean#True"
        ));
        let Ok(found) = DeviceInformation::FindAllAsyncWithKindAqsFilterAndAdditionalProperties(
            &aqs,
            &properties(&[IS_CONNECTED, CONTAINER, PROTOCOL]),
            DeviceInformationKind::AssociationEndpoint,
        )
        .and_then(|op| op.get()) else {
            return Vec::new();
        };
        let count = found.Size().unwrap_or(0);
        let mut endpoints = Vec::new();
        for i in 0..count {
            let Ok(info) = found.GetAt(i) else { continue };
            let Ok(id) = info.Id() else { continue };
            let connected = property::<bool>(&info, IS_CONNECTED).unwrap_or(false);
            let container = property::<GUID>(&info, CONTAINER);
            let low_energy = property::<GUID>(&info, PROTOCOL)
                .is_some_and(|p| format!("{{{p:?}}}").eq_ignore_ascii_case(LOW_ENERGY));
            let mut battery = None;
            if connected {
                battery = container.as_ref().and_then(container_battery);
                if battery.is_none() && low_energy {
                    battery = gatt_battery(&id);
                }
            }
            endpoints.push(Endpoint {
                id: id.to_string(),
                name: info.Name().map(|n| n.to_string()).unwrap_or_default(),
                container: container.map(|c| format!("{c:?}")),
                connected,
                battery,
            });
        }
        endpoints
    }
}

/// Paired devices now. Slow (a second or so with several devices): call it off the main thread.
pub fn list() -> Vec<BluetoothDevice> {
    #[cfg(windows)]
    return merge(win::endpoints());
    #[cfg(not(windows))]
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(
        id: &str,
        name: &str,
        container: Option<&str>,
        connected: bool,
        battery: Option<u8>,
    ) -> Endpoint {
        Endpoint {
            id: id.into(),
            name: name.into(),
            container: container.map(str::to_string),
            connected,
            battery,
        }
    }

    #[test]
    fn merges_dual_mode_devices_and_sorts_connected_first() {
        let devices = merge(vec![
            endpoint("a-classic", "Buds", Some("c1"), false, None),
            endpoint("b", "Keyboard", Some("c2"), false, None),
            endpoint("a-le", "Buds", Some("c1"), true, Some(64)),
            endpoint("c", "  ", None, true, None),
            endpoint("d", "Mouse", None, true, Some(140)),
        ]);
        let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["Buds", "Mouse", "Keyboard"]);
        assert_eq!(devices[0].id, "a-classic");
        assert!(devices[0].connected);
        assert_eq!(devices[0].battery, Some(64));
        assert_eq!(devices[1].battery, Some(100));
    }

    #[test]
    fn low_batteries_warn_once_per_discharge() {
        let device = |battery: u8, connected: bool| BluetoothDevice {
            id: "buds".into(),
            name: "Buds".into(),
            connected,
            battery: Some(battery),
        };
        let mut memory = BatteryMemory::default();
        assert!(newly_low(&[device(50, true)], &mut memory).is_empty());
        assert_eq!(newly_low(&[device(20, true)], &mut memory).len(), 1);
        assert!(newly_low(&[device(15, true)], &mut memory).is_empty());
        // Between the marks: still counts as warned.
        assert!(newly_low(&[device(25, true)], &mut memory).is_empty());
        assert!(newly_low(&[device(18, true)], &mut memory).is_empty());
        // Charged, then low again.
        assert!(newly_low(&[device(80, true)], &mut memory).is_empty());
        assert_eq!(newly_low(&[device(10, true)], &mut memory).len(), 1);
        // A device that isn't connected never warns.
        let mut fresh = BatteryMemory::default();
        assert!(newly_low(&[device(5, false)], &mut fresh).is_empty());
    }

    #[test]
    fn low_battery_cards_name_the_device() {
        let device = BluetoothDevice {
            id: "x".into(),
            name: "MX Master".into(),
            connected: true,
            battery: Some(12),
        };
        let card = low_battery_bubble(&device, Utc::now());
        assert_eq!(card.title, "MX Master battery is low");
        assert!(card.body.starts_with("12%"));
        assert_eq!(card.kind, BubbleKind::Bluetooth);
    }
}
