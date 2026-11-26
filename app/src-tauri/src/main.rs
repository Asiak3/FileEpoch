// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod settings;

fn main() {
    tauri::Builder::default()
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::load_settings,
            commands::save_settings,
            commands::probe_certificate,
            commands::connect,
            commands::disconnect,
            commands::forget_password,
            commands::list_sources,
            commands::list_snapshots,
            commands::open_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rewind");
}
