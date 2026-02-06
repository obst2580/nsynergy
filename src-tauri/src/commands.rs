use nsynergy_core::config::{AppConfig, Role, ScreenPosition};
use nsynergy_core::permissions::{self, PermissionCheck};
use nsynergy_core::security;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

/// Shared application state managed by Tauri.
pub struct AppState {
    pub config: Mutex<AppConfig>,
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

    let devices: Vec<DeviceInfo> = config
        .neighbors
        .iter()
        .map(|n| DeviceInfo {
            name: n.name.clone(),
            address: n
                .address
                .map(|a| a.to_string())
                .unwrap_or_else(|| "discovered".to_string()),
            position: position_to_string(n.position),
            connected: false,
        })
        .collect();

    let role = match config.role {
        Role::Server => "Server",
        Role::Client => "Client",
    };

    Ok(AppStateResponse {
        role: role.to_string(),
        machine_name: config.machine_name.clone(),
        connected: false,
        devices,
    })
}

#[tauri::command]
pub fn set_role(state: State<'_, AppState>, role: String) -> Result<(), String> {
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("lock error: {e}"))?;

    config.role = string_to_role(&role);

    let path = AppConfig::default_path();
    config.save(&path).map_err(|e| format!("save error: {e}"))?;

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
pub fn generate_pairing_code() -> String {
    security::generate_pairing_code()
}
