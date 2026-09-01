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

struct PluginState {
    value: String,
    tooltip: String,
    last_good_tooltip: Option<String>,
}

impl Default for PluginState {
    fn default() -> Self {
        Self {
            value: "N/A".into(),
            tooltip: "Waiting for zmk-battery-center data".into(),
            last_good_tooltip: None,
        }
    }
}

impl PluginState {
    fn refresh(&mut self) {
        match load_snapshot().and_then(render_snapshot) {
            Ok((value, tooltip)) => {
                self.value = value;
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

fn render_snapshot(snapshot: Snapshot) -> Result<(String, String), String> {
    if snapshot.schema_version != 1 {
        return Err(format!(
            "unsupported schema version {}",
            snapshot.schema_version
        ));
    }

    if snapshot.devices.is_empty() {
        return Ok(("No devices".into(), "No ZMK devices found".into()));
    }

    let multiple_devices = snapshot.devices.len() > 1;
    let mut values = Vec::new();
    let mut tooltip_lines = Vec::new();

    for device in snapshot.devices {
        let part_values: Vec<_> = device.battery_parts.iter().map(format_part_value).collect();
        let parts = if part_values.is_empty() {
            "N/A".to_string()
        } else {
            part_values.join("/")
        };

        values.push(if multiple_devices {
            format!("{}: {parts}", device.display_name)
        } else {
            parts
        });

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

    Ok((values.join(" || "), tooltip_lines.join("\n")))
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
pub extern "C" fn zmk_battery_refresh(
    value: *mut u16,
    value_capacity: usize,
    tooltip: *mut u16,
    tooltip_capacity: usize,
) -> bool {
    std::panic::catch_unwind(|| {
        let state = STATE.get_or_init(|| Mutex::new(PluginState::default()));
        let Ok(mut state) = state.lock() else {
            return false;
        };
        state.refresh();
        write_utf16(value, value_capacity, &state.value)
            && write_utf16(tooltip, tooltip_capacity, &state.tooltip)
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

        let (value, tooltip) = render_snapshot(snapshot).unwrap();

        assert_eq!(value, "87%/64%*/N/A");
        assert!(tooltip.contains("Work keyboard (connected)"));
        assert!(tooltip.contains("Right hand: 64% (stale)"));
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let snapshot = Snapshot {
            schema_version: 2,
            devices: Vec::new(),
        };

        assert_eq!(
            render_snapshot(snapshot).unwrap_err(),
            "unsupported schema version 2"
        );
    }

    #[test]
    fn writes_null_terminated_utf16() {
        let mut output = [9_u16; 4];

        assert!(write_utf16(output.as_mut_ptr(), output.len(), "ABCDE"));
        assert_eq!(output, [65, 66, 67, 0]);
    }
}
