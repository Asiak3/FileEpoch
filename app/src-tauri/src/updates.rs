//! Updates: checks GitHub Releases for a newer FileEpoch and installs it when the user agrees.
//!
//! This is the only network access besides the Kopia server, and it only happens when the user
//! keeps "check for updates" on (the default) or presses "check now". The update manifest and
//! package are verified against the public key in tauri.conf.json before anything is installed;
//! an update is never installed while a restore is running.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::commands::CmdError;
use crate::desktop::PROJECT_URL;
use crate::restore_jobs::RestoreManager;
use tauri_plugin_opener::OpenerExt as _;

/// The update found by the last check, kept until it is installed.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    current: String,
    notes: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// Returns the newer version, or `None` when FileEpoch is up to date.
#[tauri::command]
pub async fn check_update(app: AppHandle, pending: tauri::State<'_, PendingUpdate>) -> Result<Option<UpdateInfo>, CmdError> {
    let updater = app.updater().map_err(|e| CmdError::new("update_check", e.to_string()))?;
    let update = updater.check().await.map_err(|e| CmdError::new("update_check", e.to_string()))?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current: u.current_version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
    });
    *pending.0.lock().unwrap_or_else(|e| e.into_inner()) = update;
    Ok(info)
}

/// Downloads and installs the update found by `check_update`, then restarts FileEpoch.
#[tauri::command]
pub async fn install_update(app: AppHandle, pending: tauri::State<'_, PendingUpdate>) -> Result<(), CmdError> {
    if app.state::<RestoreManager>().busy() {
        return Err(CmdError::new("restore_running", "a restore is running"));
    }
    let update = pending.0.lock().unwrap_or_else(|e| e.into_inner()).take();
    let Some(update) = update else { return Err(CmdError::new("update_check", "no update to install")) };
    let mut downloaded = 0u64;
    let progress = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.emit("update-progress", UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| CmdError::new("update_install", e.to_string()))?;
    // On Windows the installer has already taken over and closed FileEpoch.
    app.restart();
}

/// Opens the release notes of `version` on GitHub.
#[tauri::command]
pub fn open_release_page(app: AppHandle, version: String) -> Result<(), CmdError> {
    let valid = !version.is_empty() && version.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    if !valid {
        return Err(CmdError::other("invalid version"));
    }
    app.opener()
        .open_url(format!("{PROJECT_URL}/releases/tag/v{version}"), None::<&str>)
        .map_err(|e| CmdError::other(e.to_string()))
}
