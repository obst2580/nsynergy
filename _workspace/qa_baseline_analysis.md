# QA Baseline Analysis

Date: 2026-04-02

## Build/Test Status

- cargo test --workspace: 124 tests ALL PASS
  - nsynergy-core: 83 tests
  - nsynergy-net: 23 tests
  - nsynergy-server: 9 tests
  - nsynergy-client: 9 tests
  - nsynergy-tauri: 0 tests (no unit tests)

## Tauri Command <-> React invoke Contract Map

### Registered Commands (src-tauri/src/lib.rs:16-24)

| # | Rust Command | Rust Return Type | React invoke<T> | Status |
|---|---|---|---|---|
| 1 | `get_app_state(State)` | `Result<AppStateResponse, String>` | `invoke<AppState>("get_app_state")` in App.tsx:39 | MATCH |
| 2 | `set_role(State, role: String)` | `Result<(), String>` | `invoke("set_role", { role: newRole })` in App.tsx:49 | MATCH |
| 3 | `get_settings(State)` | `Result<SettingsData, String>` | `invoke<SettingsData>("get_settings")` in Settings.tsx:30 | MATCH |
| 4 | `save_settings(State, settings: SettingsData)` | `Result<(), String>` | `invoke("save_settings", { settings })` in Settings.tsx:39 | MATCH |
| 5 | `check_permissions()` | `PermissionCheck` | NOT USED in frontend | N/A |
| 6 | `get_permission_instructions()` | `Vec<String>` | NOT USED in frontend | N/A |
| 7 | `generate_pairing_code()` | `String` | NOT USED in frontend | N/A |

### Type Field Mapping

**AppStateResponse (Rust) <-> AppState (React)**
- role: String <-> role: "Server" | "Client" -- MATCH
- machine_name: String <-> machine_name: string -- MATCH
- connected: bool <-> connected: boolean -- MATCH  
- devices: Vec<DeviceInfo> <-> devices: Device[] -- MATCH

**DeviceInfo (Rust) <-> Device (React)**
- name: String <-> name: string -- MATCH
- address: String <-> address: string -- MATCH
- position: String <-> position: string -- MATCH
- connected: bool <-> connected: boolean -- MATCH

**SettingsData (Rust) <-> SettingsData (React)**
- machine_name: String <-> machine_name: string -- MATCH
- udp_port: u16 <-> udp_port: number -- MATCH
- tcp_port: u16 <-> tcp_port: number -- MATCH
- edge_threshold: u32 <-> edge_threshold: number -- MATCH

## Crate Dependency Graph

```
nsynergy-core (no internal deps)
  |
  +-- nsynergy-net (depends on core: event, protocol)
  |
  +-- nsynergy-server (depends on core: config, event, screen)
  |
  +-- nsynergy-client (depends on core: event, inject, screen)
  |
  +-- nsynergy-tauri (depends on core: config, permissions, security)
```

## Inter-Crate API Boundaries

### core -> net
- `protocol::serialize_event` / `deserialize_event` used in udp.rs, tcp.rs, tls.rs
- `event::TimestampedEvent` is the wire format
- `protocol::MAX_UDP_PAYLOAD` used for buffer sizing in udp.rs

### core -> server
- `config::ScreenPosition` for neighbor lookup
- `event::{InputEvent, TimestampedEvent}` for event routing
- `screen::{DisplayInfo, ScreenEdge, detect_edge, map_position}` for edge detection

### core -> client  
- `event::{InputEvent, TimestampedEvent}` for event handling
- `inject::{InputInjector, inject_event, remap_coordinates}` for OS-level injection
- `screen::DisplayInfo` for coordinate remapping

### core -> tauri
- `config::{AppConfig, Role, ScreenPosition}` for state management
- `permissions::{check_permissions, permission_instructions, PermissionCheck}` for OS perms
- `security::generate_pairing_code` for connection auth

## Notes for Incremental QA

When other teammates add new commands or modify existing ones, verify:
1. New #[tauri::command] functions are registered in lib.rs invoke_handler
2. React invoke calls match command name exactly
3. Parameter names match (Tauri uses serde rename to snake_case)
4. Return type fields match between Rust structs and TS interfaces
5. New crate pub APIs are used correctly across boundaries
