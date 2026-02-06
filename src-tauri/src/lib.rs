mod commands;
mod tray;

use commands::AppState;
use nsynergy_core::config::AppConfig;
use std::sync::Mutex;

pub fn run() {
    let config = AppConfig::load(&AppConfig::default_path()).unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            config: Mutex::new(config),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::set_role,
            commands::get_settings,
            commands::save_settings,
            commands::check_permissions,
            commands::get_permission_instructions,
            commands::generate_pairing_code,
        ])
        .setup(|app| {
            tray::setup_tray(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
