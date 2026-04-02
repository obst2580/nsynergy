use crate::runtime::{AppRuntime, DiscoveredPeer};
use nsynergy_core::config::{AppConfig, Role, ScreenPosition};
use nsynergy_core::permissions::{self, PermissionCheck};
use nsynergy_core::security;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
#[allow(unused_imports)]
use tracing::info;
use tauri::State;

/// Shared application state managed by Tauri.
///
/// `config` and `pairing_code` use `std::sync::Mutex` (synchronous commands).
/// `runtime` uses `tokio::sync::Mutex` because async commands hold it across awaits.
pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub pairing_code: Mutex<Option<String>>,
    pub runtime: tokio::sync::Mutex<Option<AppRuntime>>,
}

/// JSON-friendly device info returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub name: String,
    pub address: String,
    pub position: String,
    pub connected: bool,
}

/// JSON-friendly app state returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateResponse {
    pub role: String,
    pub machine_name: String,
    pub connected: bool,
    pub devices: Vec<DeviceInfo>,
}

/// JSON-friendly settings for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsData {
    pub machine_name: String,
    pub udp_port: u16,
    pub tcp_port: u16,
    pub edge_threshold: u32,
}

fn position_to_string(pos: ScreenPosition) -> String {
    match pos {
        ScreenPosition::Left => "Left".to_string(),
        ScreenPosition::Right => "Right".to_string(),
        ScreenPosition::Top => "Top".to_string(),
        ScreenPosition::Bottom => "Bottom".to_string(),
    }
}

fn string_to_role(s: &str) -> Role {
    match s {
        "Client" => Role::Client,
        _ => Role::Server,
    }
}

#[tauri::command]
pub fn get_app_state(state: State<'_, AppState>) -> Result<AppStateResponse, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("lock error: {e}"))?;

    let mut rt = state
        .runtime
        .try_lock()
        .map_err(|_| "runtime busy".to_string())?;

    // Poll for status updates from backend services
    let (is_connected, runtime_devices) = if let Some(ref mut runtime) = *rt {
        runtime.poll_server_status();
        runtime.poll_client_status();
        let connected = runtime.is_connected();
        let devices: Vec<DeviceInfo> = runtime
            .connected_devices()
            .iter()
            .map(|d| DeviceInfo {
                name: d.name.clone(),
                address: d.address.clone(),
                position: "Right".to_string(),
                connected: true,
            })
            .collect();
        (connected, devices)
    } else {
        (false, Vec::new())
    };

    // Merge config neighbors with runtime devices
    let mut devices = runtime_devices;
    for n in &config.neighbors {
        let already_listed = devices.iter().any(|d| d.name == n.name);
        if !already_listed {
            devices.push(DeviceInfo {
                name: n.name.clone(),
                address: n
                    .address
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "discovered".to_string()),
                position: position_to_string(n.position),
                connected: false,
            });
        }
    }

    let role = match config.role {
        Role::Server => "Server",
        Role::Client => "Client",
    };

    Ok(AppStateResponse {
        role: role.to_string(),
        machine_name: config.machine_name.clone(),
        connected: is_connected,
        devices,
    })
}

#[tauri::command]
pub async fn set_role(state: State<'_, AppState>, role: String) -> Result<(), String> {
    let config = {
        let mut cfg = state
            .config
            .lock()
            .map_err(|e| format!("lock error: {e}"))?;
        cfg.role = string_to_role(&role);
        let path = AppConfig::default_path();
        cfg.save(&path).map_err(|e| format!("save error: {e}"))?;
        cfg.clone()
    };

    let mut rt = state.runtime.lock().await;
    let runtime = rt.get_or_insert_with(AppRuntime::new);

    match config.role {
        Role::Server => {
            runtime
                .start_as_server(&config)
                .await
                .map_err(|e| format!("failed to start server: {e}"))?;
        }
        Role::Client => {
            // When switching to client mode, stop the server
            runtime.stop().await;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<SettingsData, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("lock error: {e}"))?;

    Ok(SettingsData {
        machine_name: config.machine_name.clone(),
        udp_port: config.udp_port,
        tcp_port: config.tcp_port,
        edge_threshold: config.edge_threshold,
    })
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: SettingsData) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("lock error: {e}"))?;

    config.machine_name = settings.machine_name;
    config.udp_port = settings.udp_port;
    config.tcp_port = settings.tcp_port;
    config.edge_threshold = settings.edge_threshold;

    let path = AppConfig::default_path();
    config.save(&path).map_err(|e| format!("save error: {e}"))?;

    Ok(())
}

#[tauri::command]
pub fn check_permissions() -> PermissionCheck {
    permissions::check_permissions()
}

#[tauri::command]
pub fn get_permission_instructions() -> Vec<String> {
    let check = permissions::check_permissions();
    permissions::permission_instructions(&check)
}

#[tauri::command]
pub fn generate_pairing_code(state: State<'_, AppState>) -> Result<String, String> {
    let code = security::generate_pairing_code();
    let mut stored = state
        .pairing_code
        .lock()
        .map_err(|e| format!("lock error: {e}"))?;
    *stored = Some(code.clone());
    Ok(code)
}

#[tauri::command]
pub fn verify_pairing(state: State<'_, AppState>, code: String) -> Result<bool, String> {
    let stored = state
        .pairing_code
        .lock()
        .map_err(|e| format!("lock error: {e}"))?;
    match stored.as_ref() {
        Some(expected) => Ok(security::verify_pairing_code(expected, &code)),
        None => Err("no pairing code has been generated".to_string()),
    }
}

#[tauri::command]
pub async fn connect_device(
    state: State<'_, AppState>,
    address: String,
) -> Result<(), String> {
    let (config, addr) = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("lock error: {e}"))?;

        let addr: std::net::SocketAddr = if address.contains(':') {
            address
                .parse()
                .map_err(|e| format!("invalid address '{address}': {e}"))?
        } else {
            let ip: std::net::IpAddr = address
                .parse()
                .map_err(|e| format!("invalid IP '{address}': {e}"))?;
            std::net::SocketAddr::new(ip, config.tcp_port)
        };

        (config.clone(), addr)
    };

    tracing::info!(%addr, "connect_device: starting client connection");

    let mut rt = state.runtime.lock().await;
    let runtime = rt.get_or_insert_with(AppRuntime::new);
    runtime
        .start_as_client(addr, &config)
        .await
        .map_err(|e| format!("failed to connect: {e}"))?;

    Ok(())
}

#[tauri::command]
pub async fn scan_network(state: State<'_, AppState>) -> Result<Vec<DiscoveredPeer>, String> {
    let rt = state.runtime.lock().await;
    let runtime = rt
        .as_ref()
        .ok_or_else(|| "runtime not initialized".to_string())?;

    runtime
        .scan_network()
        .await
        .map_err(|e| format!("scan failed: {e}"))
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let mut rt = state.runtime.lock().await;

    if let Some(ref mut runtime) = *rt {
        runtime.stop().await;
    }

    Ok(())
}

// ---- Android-specific commands ----
// These are used by the mobile touchpad UI to send touch/key events
// through the Rust platform bridge to a connected desktop.

/// Send a touch-move event from the mobile touchpad UI.
#[cfg(target_os = "android")]
#[tauri::command]
pub fn mobile_touch_move(x: f64, y: f64) {
    nsynergy_core::platform::bridge_send_mouse_move(x, y);
}

/// Send a mouse button event from the mobile UI.
#[cfg(target_os = "android")]
#[tauri::command]
pub fn mobile_tap(button: u8, pressed: bool) {
    nsynergy_core::platform::bridge_send_mouse_button(button, pressed);
}

/// Send a scroll event from the mobile UI.
#[cfg(target_os = "android")]
#[tauri::command]
pub fn mobile_scroll(dx: f64, dy: f64) {
    nsynergy_core::platform::bridge_send_scroll(dx, dy);
}

/// Send a key event from the mobile virtual keyboard.
#[cfg(target_os = "android")]
#[tauri::command]
pub fn mobile_key(code: u32, pressed: bool) {
    nsynergy_core::platform::bridge_send_key(code, pressed);
}

// Desktop stubs: these commands are registered in the invoke_handler
// but are no-ops on desktop.
#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn mobile_touch_move(_x: f64, _y: f64) {}

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn mobile_tap(_button: u8, _pressed: bool) {}

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn mobile_scroll(_dx: f64, _dy: f64) {}

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn mobile_key(_code: u32, _pressed: bool) {}
