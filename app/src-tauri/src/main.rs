// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod desktop;
mod restore_jobs;
mod settings;
mod thumbnails;
mod updates;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        // First: a second start (e.g. from the Start menu while in the tray) shows this window.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // The uninstaller asks to forget the password; if FileEpoch is running it lands here.
            if args.iter().any(|a| a == desktop::FORGET_ARG) {
                desktop::forget_saved_password(app);
            } else {
                desktop::show_main(app);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![desktop::AUTOSTART_ARG]),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(commands::AppState::default())
        .manage(thumbnails::Thumbnails::default())
        .manage(desktop::TrayState::default())
        .manage(updates::PendingUpdate::default())
        .on_window_event(desktop::on_window_event)
        .setup(|app| {
            // Uninstalling with "delete the application data": remove the saved password and quit
            // before the window or the connection start.
            if std::env::args().any(|a| a == desktop::FORGET_ARG) {
                desktop::forget_saved_password(app.handle());
                std::process::exit(0);
            }
            let manager = restore_jobs::RestoreManager::start(app.handle());
            app.manage(manager);
            // The window starts hidden (tauri.conf.json): shown now, unless FileEpoch was started
            // with the system and lives in the tray.
            desktop::migrate_autostart(app.handle());
            let settings = settings::load(app.handle());
            let locale =
                if matches!(settings.language.as_str(), "es" | "de") { settings.language.as_str() } else { "en" };
            let tray_ok = settings.tray && desktop::set_tray(app.handle(), true, locale).is_ok();
            let autostarted = std::env::args().any(|a| a == desktop::AUTOSTART_ARG);
            if !(tray_ok && autostarted) {
                desktop::show_main(app.handle());
            }
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
            thumbnails::thumbnail,
            desktop::configure_tray,
            desktop::quit_app,
            desktop::get_autostart,
            desktop::set_autostart,
            desktop::open_project_page,
            desktop::open_licenses,
            updates::check_update,
            updates::install_update,
            updates::open_release_page,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FileEpoch");
}
