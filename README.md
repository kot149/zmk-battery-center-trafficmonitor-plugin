# zmk-battery-center TrafficMonitor plugin

A [TrafficMonitor](https://github.com/zhongyang219/TrafficMonitor) plugin that displays the battery snapshot published by [zmk-battery-center](https://github.com/kot149/zmk-battery-center).

The snapshot is loaded when the plugin initializes, and refreshed in `DataRequired()`. TrafficMonitor's frequently called value getter only returns cached display text.

## Display

The plugin creates one item per device when TrafficMonitor loads it. Each item uses the device ID from the snapshot as its stable ID and is labeled with the device display name.

```text
ZMK: Corne    87%/64%*
ZMK: Temper   92%/80%
```

The value lists that device's battery levels in snapshot order, separated by `/`. `*` marks a stale last-known value. The tooltip shows full part names, device connection states, and the `current`, `stale`, or `unavailable` status of every battery part.

When a snapshot read fails, the last good values remain visible and the tooltip reports the error. Changes to the device list require restarting TrafficMonitor.

## Build

Requirements:

- Windows
- Rust with the `x86_64-pc-windows-msvc` target
- Visual Studio Build Tools with C++ support

```powershell
cargo build --release
```

The DLL is created at:

```text
target\release\zmk_battery_center_trafficmonitor_plugin.dll
```

Copy it into TrafficMonitor's `plugins` directory, then restart TrafficMonitor.

## Snapshot path

The default input is:

```text
%APPDATA%\com.zmk-battery-center.app\external\battery-state-v1.json
```

For development, set `ZMK_BATTERY_CENTER_DATA_DIR` to the same data directory used by a debug build of zmk-battery-center. The plugin appends `external\battery-state-v1.json` to that directory.

## Implementation

TrafficMonitor exposes a C++ virtual-class plugin ABI. The snapshot reader and formatter are implemented in Rust, while `native/plugin.cpp` is a minimal ABI shim built with MSVC.

`include/PluginInterface.h` is copied from the official TrafficMonitor repository and remains subject to its upstream license.
