// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod restore_jobs;
mod settings;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppState::default())
        .setup(|app| {
            let manager = restore_jobs::RestoreManager::start(app.handle());
            app.manage(manager);
            Ok(())
        })
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
            restore_jobs::choose_destination,
            restore_jobs::restore_conflicts,
            restore_jobs::start_restore,
            restore_jobs::list_restores,
            restore_jobs::cancel_restore,
            restore_jobs::clear_restores,
            restore_jobs::open_restore_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rewind");
}
