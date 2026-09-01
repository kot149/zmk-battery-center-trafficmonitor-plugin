use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::sync::{Mutex, OnceLock};

const SNAPSHOT_FILENAME: &str = "battery-state-v1.json";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    schema_version: u8,
    devices: Vec<Device>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    id: String,
    display_name: String,
    connection_status: String,
    battery_parts: Vec<BatteryPart>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatteryPart {
    display_name: String,
    level_percent: Option<u8>,
    value_status: String,
}

struct DeviceState {
    id: String,
    display_name: String,
    value: String,
}

struct PluginState {
    devices: Vec<DeviceState>,
    tooltip: String,
    last_good_tooltip: Option<String>,
    initialized: bool,
}

impl Default for PluginState {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
            tooltip: "Waiting for zmk-battery-center data".into(),
            last_good_tooltip: None,
            initialized: false,
        }
    }
}

impl PluginState {
    fn initialize(&mut self) {
        if self.initialized {
            return;
        }

        self.initialized = true;
        match load_snapshot().and_then(validate_snapshot) {
            Ok(snapshot) => {
                self.devices = snapshot
                    .devices
                    .iter()
                    .map(|device| DeviceState {
                        id: device.id.clone(),
                        display_name: device.display_name.clone(),
                        value: format_device_value(device),
                    })
                    .collect();
                let tooltip = render_tooltip(&snapshot.devices);
                self.tooltip = tooltip.clone();
                self.last_good_tooltip = Some(tooltip);
            }
            Err(error) => {
                self.tooltip = format!("Snapshot read error: {error}");
            }
        }
    }

    fn refresh(&mut self) {
        self.initialize();
        match load_snapshot().and_then(validate_snapshot) {
            Ok(snapshot) => {
                for device in &mut self.devices {
                    device.value = snapshot
                        .devices
                        .iter()
                        .find(|current| current.id == device.id)
                        .map_or_else(|| "N/A".into(), format_device_value);
                }
                let tooltip = render_tooltip(&snapshot.devices);
                self.tooltip = tooltip.clone();
                self.last_good_tooltip = Some(tooltip);
            }
            Err(error) => {
                self.tooltip = match &self.last_good_tooltip {
                    Some(tooltip) => format!("{tooltip}\n\nSnapshot read error: {error}"),
                    None => format!("Snapshot read error: {error}"),
                };
            }
        }
    }
}

static STATE: OnceLock<Mutex<PluginState>> = OnceLock::new();

fn snapshot_path() -> Result<PathBuf, String> {
    let base = match env::var_os("ZMK_BATTERY_CENTER_DATA_DIR") {
        Some(path) => PathBuf::from(path),
        None => env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| "APPDATA is not set".to_string())?
            .join("com.zmk-battery-center.app"),
    };

    Ok(base.join("external").join(SNAPSHOT_FILENAME))
}

fn load_snapshot() -> Result<Snapshot, String> {
    let path = snapshot_path()?;
    let contents =
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_str(&contents).map_err(|error| format!("{}: {error}", path.display()))
}

fn validate_snapshot(snapshot: Snapshot) -> Result<Snapshot, String> {
    if snapshot.schema_version != 1 {
        return Err(format!(
            "unsupported schema version {}",
            snapshot.schema_version
        ));
    }

    Ok(snapshot)
}

fn format_device_value(device: &Device) -> String {
    let part_values: Vec<_> = device.battery_parts.iter().map(format_part_value).collect();
    if part_values.is_empty() {
        "N/A".into()
    } else {
        part_values.join("/")
    }
}

fn render_tooltip(devices: &[Device]) -> String {
    if devices.is_empty() {
        return "No ZMK devices found".into();
    }

    let mut tooltip_lines = Vec::new();
    for device in devices {
        tooltip_lines.push(format!(
            "{} ({})",
            device.display_name, device.connection_status
        ));
        if device.battery_parts.is_empty() {
            tooltip_lines.push("  No battery parts".into());
        } else {
            tooltip_lines.extend(device.battery_parts.iter().map(|part| {
                format!(
                    "  {}: {} ({})",
                    part.display_name,
                    format_level(part.level_percent),
                    part.value_status
                )
            }));
        }
    }

    tooltip_lines.join("\n")
}

fn format_part_value(part: &BatteryPart) -> String {
    let suffix = if part.value_status == "stale" {
        "*"
    } else {
        ""
    };
    format!("{}{suffix}", format_level(part.level_percent))
}

fn format_level(level: Option<u8>) -> String {
    level.map_or_else(|| "N/A".into(), |level| format!("{level}%"))
}

fn write_utf16(output: *mut u16, capacity: usize, value: &str) -> bool {
    if output.is_null() || capacity == 0 {
        return false;
    }

    let encoded: Vec<_> = value.encode_utf16().take(capacity - 1).collect();
    unsafe {
        ptr::copy_nonoverlapping(encoded.as_ptr(), output, encoded.len());
        output.add(encoded.len()).write(0);
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn zmk_battery_device_count() -> usize {
    std::panic::catch_unwind(|| {
        let state = STATE.get_or_init(|| Mutex::new(PluginState::default()));
        let Ok(mut state) = state.lock() else {
            return 0;
        };
        state.initialize();
        state.devices.len()
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn zmk_battery_device_info(
    index: usize,
    id: *mut u16,
    id_capacity: usize,
    display_name: *mut u16,
    display_name_capacity: usize,
) -> bool {
    std::panic::catch_unwind(|| {
        let state = STATE.get_or_init(|| Mutex::new(PluginState::default()));
        let Ok(mut state) = state.lock() else {
            return false;
        };
        state.initialize();
        let Some(device) = state.devices.get(index) else {
            return false;
        };
        write_utf16(id, id_capacity, &device.id)
            && write_utf16(display_name, display_name_capacity, &device.display_name)
    })
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn zmk_battery_refresh() -> bool {
    std::panic::catch_unwind(|| {
        let state = STATE.get_or_init(|| Mutex::new(PluginState::default()));
        let Ok(mut state) = state.lock() else {
            return false;
        };
        state.refresh();
        true
    })
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn zmk_battery_device_value(
    index: usize,
    value: *mut u16,
    value_capacity: usize,
) -> bool {
    std::panic::catch_unwind(|| {
        let state = STATE.get_or_init(|| Mutex::new(PluginState::default()));
        let Ok(state) = state.lock() else {
            return false;
        };
        let Some(device) = state.devices.get(index) else {
            return false;
        };
        write_utf16(value, value_capacity, &device.value)
    })
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn zmk_battery_tooltip(tooltip: *mut u16, tooltip_capacity: usize) -> bool {
    std::panic::catch_unwind(|| {
        let state = STATE.get_or_init(|| Mutex::new(PluginState::default()));
        let Ok(state) = state.lock() else {
            return false;
        };
        write_utf16(tooltip, tooltip_capacity, &state.tooltip)
    })
    .unwrap_or(false)
}

#[cfg(windows)]
unsafe extern "C" {
    fn zmk_tm_link_anchor();
}

#[cfg(windows)]
#[used]
static LINK_TRAFFICMONITOR_SHIM: unsafe extern "C" fn() = zmk_tm_link_anchor;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_current_stale_and_unavailable_parts() {
        let snapshot: Snapshot = serde_json::from_str(
            r#"{
                "schemaVersion": 1,
                "devices": [{
                    "id": "work-keyboard",
                    "displayName": "Work keyboard",
                    "connectionStatus": "connected",
                    "batteryParts": [
                        {"displayName": "Central", "levelPercent": 87, "valueStatus": "current"},
                        {"displayName": "Right hand", "levelPercent": 64, "valueStatus": "stale"},
                        {"displayName": "Left hand", "levelPercent": null, "valueStatus": "unavailable"}
                    ]
                }]
            }"#,
        )
        .unwrap();

        let value = format_device_value(&snapshot.devices[0]);
        let tooltip = render_tooltip(&snapshot.devices);

        assert_eq!(value, "87%/64%*/N/A");
        assert!(tooltip.contains("Work keyboard (connected)"));
        assert!(tooltip.contains("Right hand: 64% (stale)"));
    }

    #[test]
    fn renders_multiple_devices_in_input_order() {
        let snapshot: Snapshot = serde_json::from_str(
            r#"{
                "schemaVersion": 1,
                "devices": [
                    {
                        "id": "desk",
                        "displayName": "Desk",
                        "connectionStatus": "connected",
                        "batteryParts": [
                            {"displayName": "Central", "levelPercent": 87, "valueStatus": "current"}
                        ]
                    },
                    {
                        "id": "travel",
                        "displayName": "Travel",
                        "connectionStatus": "disconnected",
                        "batteryParts": []
                    }
                ]
            }"#,
        )
        .unwrap();

        let values: Vec<_> = snapshot.devices.iter().map(format_device_value).collect();
        let tooltip = render_tooltip(&snapshot.devices);

        assert_eq!(values, vec!["87%".to_string(), "N/A".to_string()]);
        assert_eq!(
            tooltip,
            "Desk (connected)\n  Central: 87% (current)\nTravel (disconnected)\n  No battery parts"
        );
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let snapshot = Snapshot {
            schema_version: 2,
            devices: Vec::new(),
        };

        assert_eq!(
            validate_snapshot(snapshot).err(),
            Some("unsupported schema version 2".into())
        );
    }

    #[test]
    fn writes_null_terminated_utf16() {
        let mut output = [9_u16; 4];

        assert!(write_utf16(output.as_mut_ptr(), output.len(), "ABCDE"));
        assert_eq!(output, [65, 66, 67, 0]);
    }
}
