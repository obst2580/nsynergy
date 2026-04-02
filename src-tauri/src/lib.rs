mod commands;
pub mod runtime;

#[cfg(desktop)]
mod tray;

use commands::AppState;
use nsynergy_core::config::AppConfig;
use std::sync::Mutex;

pub fn run() {
    let config = AppConfig::load(&AppConfig::default_path()).unwrap_or_default();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            config: Mutex::new(config),
            pairing_code: Mutex::new(None),
            runtime: tokio::sync::Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::set_role,
            commands::get_settings,
            commands::save_settings,
            commands::check_permissions,
            commands::get_permission_instructions,
            commands::generate_pairing_code,
            commands::verify_pairing,
            commands::connect_device,
            commands::scan_network,
            commands::disconnect,
            commands::mobile_touch_move,
            commands::mobile_tap,
            commands::mobile_scroll,
            commands::mobile_key,
        ]);

    #[cfg(desktop)]
    let builder = builder.setup(|app| {
        tray::setup_tray(app)?;
        Ok(())
    });

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
