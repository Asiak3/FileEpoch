//! Desktop integration: system tray, start with the operating system, single window, quitting
//! safely while restores run, and the project page link.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_opener::OpenerExt as _;

use crate::commands::CmdError;
use crate::restore_jobs::RestoreManager;

/// Public home of the project (shown as "by A.C." in the app).
pub const PROJECT_URL: &str = "https://github.com/Asiak3/FileEpoch";
/// Passed by the operating system when FileEpoch starts with it.
pub const AUTOSTART_ARG: &str = "--autostart";
const TRAY_ID: &str = "fileepoch";

/// Whether closing the window keeps FileEpoch in the tray.
#[derive(Default)]
pub struct TrayState {
    enabled: AtomicBool,
}

fn labels(locale: &str) -> (&'static str, &'static str) {
    match locale {
        "es" => ("Abrir FileEpoch", "Salir"),
        "de" => ("FileEpoch öffnen", "Beenden"),
        _ => ("Open FileEpoch", "Quit"),
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Quits, unless restores are running: then the window asks first (`quit-requested`).
fn request_quit(app: &AppHandle) {
    if app.state::<RestoreManager>().busy() {
        show_main(app);
        let _ = app.emit("quit-requested", ());
    } else {
        app.exit(0);
    }
}

/// Shows or removes the tray icon. `locale` is the UI language (es, de, en).
pub fn set_tray(app: &AppHandle, enabled: bool, locale: &str) -> tauri::Result<()> {
    app.state::<TrayState>().enabled.store(enabled, Ordering::Relaxed);
    if !enabled {
        let _ = app.remove_tray_by_id(TRAY_ID);
        return Ok(());
    }
    let (open, quit) = labels(locale);
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", open, true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", quit, true, None::<&str>)?,
        ],
    )?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        return tray.set_menu(Some(menu));
    }
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("FileEpoch")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Closing the window: hide it to the tray when enabled; otherwise quit (asking if restores run).
pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        let app = window.app_handle();
        if app.state::<TrayState>().enabled.load(Ordering::Relaxed) {
            api.prevent_close();
            let _ = window.hide();
        } else if app.state::<RestoreManager>().busy() {
            api.prevent_close();
            let _ = app.emit("quit-requested", ());
        }
    }
}

#[tauri::command]
pub fn configure_tray(app: AppHandle, enabled: bool, locale: String) -> Result<(), CmdError> {
    set_tray(&app, enabled, &locale).map_err(|e| CmdError::other(format!("tray: {e}")))
}

/// Quit after the user confirmed: cancel restores and give them a moment to clean up.
#[tauri::command]
pub async fn quit_app(app: AppHandle) {
    let manager = app.state::<RestoreManager>();
    manager.cancel_all();
    let deadline = Instant::now() + Duration::from_secs(3);
    while manager.busy() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    app.exit(0);
}

/// Earlier versions were called "Rewind": if they were set to start with the system, remove that
/// entry (it points to the old program) and enable the current one instead.
pub fn migrate_autostart(app: &AppHandle) {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(exe) = exe.to_str() else { return };
    let Ok(old) = auto_launch::AutoLaunchBuilder::new().set_app_name("Rewind").set_app_path(exe).build() else {
        return;
    };
    if old.is_enabled().unwrap_or(false) && old.disable().is_ok() {
        let _ = app.autolaunch().enable();
    }
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, CmdError> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| CmdError::other(format!("start with the system: {e}")))?;
    Ok(launcher.is_enabled().unwrap_or(false))
}

#[tauri::command]
pub fn open_project_page(app: AppHandle) -> Result<(), CmdError> {
    app.opener().open_url(PROJECT_URL, None::<&str>).map_err(|e| CmdError::other(e.to_string()))
}
