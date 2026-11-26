//! Commands invoked by the user interface.
//!
//! All server access happens here, through `kopia_client` (whitelisted read-only requests,
//! pinned certificate). The password only travels from the UI to this backend and is never
//! returned or written anywhere except the system credential store.

use std::sync::{Arc, Mutex};

use kopia_client::{
    EntryType, Error, Fingerprint, KopiaClient, Oid, ServerAddress, Snapshot, SourceInfo, Warning, credentials,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::settings::{self, Settings};

#[derive(Default)]
pub struct AppState {
    client: Mutex<Option<Arc<KopiaClient>>>,
}

impl AppState {
    fn client(&self) -> Result<Arc<KopiaClient>, CmdError> {
        self.client
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| CmdError::new("not_connected_app", "not connected"))
    }

    fn set(&self, client: Option<Arc<KopiaClient>>) {
        *self.client.lock().unwrap_or_else(|e| e.into_inner()) = client;
    }
}

/// Error sent to the UI. `code` is stable and translated by the UI; `message` is for details.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdError {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    presented: Option<String>,
}

impl CmdError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        CmdError { code, message: message.into(), presented: None }
    }
}

impl From<Error> for CmdError {
    fn from(e: Error) -> Self {
        let message = e.to_string();
        let (code, presented) = match e {
            Error::InvalidServerAddress(_) => ("invalid_server", None),
            Error::InvalidFingerprint => ("invalid_fingerprint", None),
            Error::CertificateMismatch { presented } => ("certificate_mismatch", Some(presented)),
            Error::Connection(_) => ("connection", None),
            Error::AccessDenied => ("access_denied", None),
            Error::NotUiUser => ("not_ui_user", None),
            Error::CsrfRejected | Error::MissingCredentials => ("session", None),
            Error::NotConnected => ("server_not_connected", None),
            Error::NotFound(_) => ("not_found", None),
            Error::Incompatible { .. } => ("incompatible", None),
            Error::Credentials(_) => ("credential_store", None),
            _ => ("other", None),
        };
        CmdError { code, message, presented }
    }
}

type CmdResult<T> = Result<T, CmdError>;

// --- settings ------------------------------------------------------------------------------------

#[tauri::command]
pub fn load_settings(app: tauri::AppHandle) -> Settings {
    settings::load(&app)
}

#[tauri::command]
pub fn save_settings(app: tauri::AppHandle, settings: Settings) -> CmdResult<()> {
    settings::save(&app, &settings).map_err(|e| CmdError::new("other", e))
}

// --- connection ----------------------------------------------------------------------------------

#[derive(Serialize)]
pub struct CertInfo {
    hex: String,
    colon: String,
}

/// Reads the server certificate fingerprint without sending any HTTP request.
#[tauri::command]
pub async fn probe_certificate(server: String) -> CmdResult<CertInfo> {
    let server: ServerAddress = server.parse()?;
    let fp = kopia_client::probe_certificate(&server).await?;
    Ok(CertInfo { hex: fp.to_hex(), colon: fp.to_colon_hex() })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRequest {
    server: String,
    user: String,
    fingerprint: String,
    /// `None`: use the password saved in the credential store.
    password: Option<String>,
    remember: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResult {
    version: Option<String>,
    tested_versions: Vec<&'static str>,
    untested_version: bool,
    readonly: bool,
    description: String,
}

#[tauri::command]
pub async fn connect(state: tauri::State<'_, AppState>, request: ConnectRequest) -> CmdResult<ConnectResult> {
    let ConnectRequest { server, user, fingerprint, password, remember } = request;
    let password = password.map(Zeroizing::new);
    let server: ServerAddress = server.parse()?;
    let fingerprint: Fingerprint = fingerprint.parse()?;
    state.set(None);

    // Check the certificate before any credential is used.
    let presented = kopia_client::probe_certificate(&server).await?;
    if presented != fingerprint {
        return Err(Error::CertificateMismatch { presented: presented.to_hex() }.into());
    }

    let (password, from_store) = match password {
        Some(p) => (p, false),
        None => match credentials::load(&server, &user)? {
            Some(p) => (p, true),
            None => return Err(CmdError::new("password_required", "no saved password")),
        },
    };

    let client = KopiaClient::new(server.clone(), &user, &password, fingerprint)?;
    let info = match client.connect().await {
        Ok(info) => info,
        Err(Error::AccessDenied) if from_store => {
            return Err(CmdError::new("saved_password_rejected", "the saved password was rejected"));
        }
        Err(e) => return Err(e.into()),
    };
    if remember && !from_store {
        credentials::store(&server, &user, &password)?;
    }
    state.set(Some(Arc::new(client)));

    Ok(ConnectResult {
        version: info.version,
        tested_versions: kopia_client::TESTED_VERSIONS.to_vec(),
        untested_version: info.warnings.iter().any(|w| matches!(w, Warning::UntestedVersion(_))),
        readonly: info.repo.readonly,
        description: info.repo.description,
    })
}

#[tauri::command]
pub fn disconnect(state: tauri::State<'_, AppState>) {
    state.set(None);
}

#[tauri::command]
pub fn forget_password(server: String, user: String) -> CmdResult<bool> {
    let server: ServerAddress = server.parse()?;
    Ok(credentials::delete(&server, &user)?)
}

// --- browsing ------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SourceDto {
    host: String,
    user_name: String,
    path: String,
}

impl From<SourceInfo> for SourceDto {
    fn from(s: SourceInfo) -> Self {
        SourceDto { host: s.host, user_name: s.user_name, path: s.path }
    }
}

#[tauri::command]
pub async fn list_sources(state: tauri::State<'_, AppState>) -> CmdResult<Vec<SourceDto>> {
    let client = state.client()?;
    let mut sources: Vec<SourceDto> = client.sources().await?.sources.into_iter().map(|s| s.source.into()).collect();
    sources.sort_by(|a, b| (&a.user_name, &a.host, &a.path).cmp(&(&b.user_name, &b.host, &b.path)));
    Ok(sources)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDto {
    id: String,
    /// RFC 3339, UTC. The UI formats it in local time.
    start_time: String,
    end_time: Option<String>,
    root_id: String,
    incomplete: Option<String>,
    files: u64,
    size: u64,
    retention: Vec<String>,
    pins: Vec<String>,
    description: String,
}

impl From<Snapshot> for SnapshotDto {
    fn from(s: Snapshot) -> Self {
        let (files, size) = s.summary.as_ref().map(|x| (x.files, x.size)).unwrap_or_default();
        SnapshotDto {
            id: s.id,
            start_time: s.start_time.to_rfc3339(),
            end_time: s.end_time.map(|t| t.to_rfc3339()),
            root_id: s.root_id.to_string(),
            incomplete: s.incomplete,
            files,
            size,
            retention: s.retention,
            pins: s.pins,
            description: s.description,
        }
    }
}

/// All snapshots of a source, oldest first. Grouping is done by the UI (consecutive runs with
/// the same root), see docs/API.md §5.
#[tauri::command]
pub async fn list_snapshots(state: tauri::State<'_, AppState>, source: SourceDto) -> CmdResult<Vec<SnapshotDto>> {
    let client = state.client()?;
    let info = SourceInfo { host: source.host, user_name: source.user_name, path: source.path };
    Ok(client.snapshots(&info, true).await?.snapshots.into_iter().map(Into::into).collect())
}

/// Compact directory entry for the file list.
#[derive(Serialize)]
pub struct EntryDto {
    name: String,
    /// `f` file, `d` directory, `s` symlink, `?` unknown.
    kind: &'static str,
    /// File size, or total size of a directory's contents.
    size: u64,
    /// Modification time in milliseconds since the epoch (UTC), if known.
    mtime: Option<i64>,
    obj: String,
    /// Number of files below a directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<u64>,
}

#[derive(Serialize)]
pub struct ListingDto {
    /// How many components of the requested path exist in this snapshot.
    depth: usize,
    entries: Vec<EntryDto>,
}

/// Opens `path` (folder names) in the snapshot whose root is `root`, or its deepest existing
/// ancestor (see `KopiaClient::resolve_path`).
#[tauri::command]
pub async fn open_path(state: tauri::State<'_, AppState>, root: String, path: Vec<String>) -> CmdResult<ListingDto> {
    let client = state.client()?;
    let root: Oid = root.parse()?;
    let r = client.resolve_path(&root, &path).await?;
    let entries = r
        .listing
        .entries
        .iter()
        .map(|e| EntryDto {
            name: e.name.clone(),
            kind: match e.entry_type {
                EntryType::File => "f",
                EntryType::Directory => "d",
                EntryType::Symlink => "s",
                EntryType::Unknown => "?",
            },
            size: match e.entry_type {
                EntryType::Directory => e.summ.as_ref().map(|s| s.size).unwrap_or(0),
                _ => e.size,
            },
            mtime: e.mtime.map(|t| t.timestamp_millis()),
            obj: e.obj.to_string(),
            files: e.summ.as_ref().map(|s| s.files),
        })
        .collect();
    Ok(ListingDto { depth: r.depth, entries })
}
