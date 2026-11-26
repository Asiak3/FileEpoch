//! Non-secret settings, stored as JSON in the app's config folder
//! (`%APPDATA%\io.github.asiak3.rewind\settings.json` on Windows). Passwords are never stored here.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub server: String,
    pub user: String,
    /// Trusted SHA-256 fingerprint of the server certificate (Kopia hex format).
    pub fingerprint: String,
    /// `auto`, `es`, `de` or `en`.
    pub language: String,
    /// `system`, `light` or `dark`.
    pub theme: String,
    /// Group consecutive snapshots with identical content on the timeline.
    pub group_identical: bool,
    /// Last selected source, as `user@host:path`.
    pub last_source: String,
    /// File view: `details`, `list`, `icons` or `tiles`.
    pub view_mode: String,
    /// Optional columns of the details view, in display order (empty: defaults).
    pub columns: Vec<ColumnSetting>,
    /// Restore: `suffix` (add "(version of <date>)") or `original`.
    pub restore_naming: String,
    /// Restore: date in the suffix, `snapshot` or `modified`.
    pub suffix_date: String,
    /// Restore: `rename` (keep both), `skip` or `overwrite` (always asks first).
    pub conflict: String,
    /// Restore: check size and date of every restored file.
    pub verify: bool,
    /// Folder offered first by the destination picker.
    pub last_restore_dir: String,
    /// Show image thumbnails in the icon views (downloads the images).
    pub thumbnails: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSetting {
    pub id: String,
    pub visible: bool,
    pub width: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            server: String::new(),
            user: String::new(),
            fingerprint: String::new(),
            language: "auto".into(),
            theme: "system".into(),
            group_identical: true,
            last_source: String::new(),
            view_mode: "details".into(),
            columns: Vec::new(),
            restore_naming: "suffix".into(),
            suffix_date: "snapshot".into(),
            conflict: "rename".into(),
            verify: true,
            last_restore_dir: String::new(),
            thumbnails: true,
        }
    }
}

fn path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load(app: &tauri::AppHandle) -> Settings {
    path(app).ok().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(app: &tauri::AppHandle, settings: &Settings) -> Result<(), String> {
    let p = path(app)?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}
