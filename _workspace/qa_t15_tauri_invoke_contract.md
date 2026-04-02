# T15: Tauri Command <-> React invoke Contract Verification Report

Date: 2026-04-02
Status: PASS (all contracts verified)

## Summary

All 7 Tauri commands registered in `src-tauri/src/lib.rs:16-24` have been verified against their React frontend invoke calls. No contract mismatches found.

## Detailed Verification

### 1. get_app_state -- PASS

**Producer (Rust)**: `src-tauri/src/commands.rs:57`
```rust
#[tauri::command]
pub fn get_app_state(state: State<'_, AppState>) -> Result<AppStateResponse, String>
```
- AppStateResponse: { role: String, machine_name: String, connected: bool, devices: Vec<DeviceInfo> }
- DeviceInfo: { name: String, address: String, position: String, connected: bool }

**Consumer (React)**: `src/App.tsx:28`
```ts
const result = await invoke<AppState>("get_app_state");
```
- AppState: { role: "Server" | "Client", machine_name: string, connected: boolean, devices: Device[] }
- Device: { name: string, address: string, position: string, connected: boolean }

**Verdict**: MATCH -- all field names and types align. React uses stricter union type for role ("Server"|"Client") which is a subset of String -- safe.

### 2. set_role -- PASS

**Producer (Rust)**: `src-tauri/src/commands.rs:91`
```rust
#[tauri::command]
pub fn set_role(state: State<'_, AppState>, role: String) -> Result<(), String>
```

**Consumer (React)**: `src/App.tsx:39`
```ts
await invoke("set_role", { role: newRole });
```

**Verdict**: MATCH -- parameter name `role` matches. Value is "Server" or "Client" which Rust handles via string_to_role().

### 3. get_settings -- PASS

**Producer (Rust)**: `src-tauri/src/commands.rs:106`
```rust
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<SettingsData, String>
```
- SettingsData: { machine_name: String, udp_port: u16, tcp_port: u16, edge_threshold: u32 }

**Consumer (React)**: `src/components/Settings.tsx:30`
```ts
const result = await invoke<SettingsData>("get_settings");
```
- SettingsData: { machine_name: string, udp_port: number, tcp_port: number, edge_threshold: number }

**Verdict**: MATCH -- u16/u32 serialize to JSON numbers which map to TS number.

### 4. save_settings -- PASS

**Producer (Rust)**: `src-tauri/src/commands.rs:121`
```rust
#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: SettingsData) -> Result<(), String>
```

**Consumer (React)**: `src/components/Settings.tsx:52`
```ts
await invoke("save_settings", { settings });
```

**Verdict**: MATCH -- parameter name `settings` matches. Tauri deserializes the JS object into Rust SettingsData via serde. All field names match.

### 5. check_permissions -- PASS

**Producer (Rust)**: `src-tauri/src/commands.rs:139`
```rust
#[tauri::command]
pub fn check_permissions() -> PermissionCheck
```
- PermissionCheck: { accessibility: PermissionStatus, input_monitoring: PermissionStatus }
- PermissionStatus enum: Granted | Denied | NotApplicable (serde serializes as strings)

**Consumer (React)**: `src/components/Settings.tsx:40`
```ts
invoke<PermissionCheck>("check_permissions")
```
- PermissionCheck: { accessibility: "Granted"|"Denied"|"NotApplicable", input_monitoring: "Granted"|"Denied"|"NotApplicable" }

**Verdict**: MATCH -- serde serializes Rust enum variants as strings matching the TS union type exactly.

**Note**: This command returns PermissionCheck directly (not wrapped in Result). Tauri handles this correctly by auto-wrapping non-Result returns.

### 6. get_permission_instructions -- PASS

**Producer (Rust)**: `src-tauri/src/commands.rs:144`
```rust
#[tauri::command]
pub fn get_permission_instructions() -> Vec<String>
```

**Consumer (React)**: `src/components/Settings.tsx:41`
```ts
invoke<string[]>("get_permission_instructions")
```

**Verdict**: MATCH -- Vec<String> serializes to JSON string array, matching TS string[].

### 7. generate_pairing_code -- PASS

**Producer (Rust)**: `src-tauri/src/commands.rs:150`
```rust
#[tauri::command]
pub fn generate_pairing_code() -> String
```

**Consumer (React)**: `src/components/PairingDialog.tsx:26`
```ts
const code = await invoke<string>("generate_pairing_code");
```

**Verdict**: MATCH -- String serializes to JSON string.

## Type Source Verification

All React types are centralized in `src/types.ts` and imported by components:
- App.tsx imports: AppState from types.ts
- Settings.tsx imports: SettingsData, PermissionCheck from types.ts
- DeviceList.tsx imports: Device, ConnectionStatus from types.ts
- ScreenLayout.tsx imports: Device from types.ts
- ConnectionIndicator.tsx imports: ConnectionStatus from types.ts
- PairingDialog.tsx: inline types (no shared types needed)

## invoke_handler Registration Check

All 7 commands in `src-tauri/src/lib.rs:16-24` are registered:
```rust
.invoke_handler(tauri::generate_handler![
    commands::get_app_state,       // used in App.tsx
    commands::set_role,            // used in App.tsx
    commands::get_settings,        // used in Settings.tsx
    commands::save_settings,       // used in Settings.tsx
    commands::check_permissions,   // used in Settings.tsx
    commands::get_permission_instructions, // used in Settings.tsx
    commands::generate_pairing_code,      // used in PairingDialog.tsx
])
```

No unregistered commands are invoked from the frontend. No registered commands are missing frontend consumers (all 7 are now used).

## Potential Future Considerations

1. PairingDialog.tsx:76 has a TODO comment about `invoke("verify_pairing", { code, deviceName })` -- this command does not exist yet in Rust. Currently simulated client-side. When implemented, needs to be added to both commands.rs and invoke_handler.

2. DeviceList.tsx:35 has a TODO comment about `invoke("connect_device", { address })` -- not yet implemented in Rust. Currently simulated.

3. App.tsx:23 polls `get_app_state` every 5 seconds via setInterval. This is fine for now but should be replaced with Tauri events for real-time updates.
