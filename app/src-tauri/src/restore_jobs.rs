//! Restore queue: jobs run one at a time in the background and report progress to the UI with
//! `restore-progress` events. Restores always write to the local computer (see
//! `kopia_client::restore`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::{DateTime, Local, Utc};
use kopia_client::restore::{self, ConflictPolicy, Naming, Phase, Progress, RestoreOptions, SuffixDate, Summary};
use kopia_client::{DirEntry, EntryType, KopiaClient, Oid};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::commands::{AppState, CmdError, SourceDto};

/// At most this many names per list are sent to the UI (the counts are always exact).
const LIST_LIMIT: usize = 200;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRequest {
    source: SourceDto,
    /// Root object of the snapshot shown when the user asked to restore.
    snapshot_root: String,
    /// Start time of that snapshot (RFC 3339).
    snapshot_time: String,
    /// Folder shown at that moment (names below the snapshot root).
    path: Vec<String>,
    /// Selected names in that folder; empty = the folder itself.
    names: Vec<String>,
    dest: String,
    naming: String,
    suffix_date: String,
    /// Translated words before the date in the suffix, e.g. "versión del" or "modificado".
    label: String,
    conflict: String,
    /// Must be true for `conflict == "overwrite"`; set only after the user confirmed.
    overwrite_confirmed: bool,
    verify: bool,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SummaryDto {
    top_level: Vec<String>,
    files_restored: u64,
    bytes_restored: u64,
    dirs_created: u64,
    renamed: Vec<[String; 2]>,
    renamed_count: usize,
    skipped_links: Vec<String>,
    skipped_links_count: usize,
    skipped_existing: Vec<String>,
    skipped_existing_count: usize,
    errors: Vec<[String; 2]>,
    errors_count: usize,
}

impl From<&Summary> for SummaryDto {
    fn from(s: &Summary) -> Self {
        let cap = |v: &Vec<String>| v.iter().take(LIST_LIMIT).cloned().collect();
        SummaryDto {
            top_level: s.top_level.iter().map(|p| p.display().to_string()).collect(),
            files_restored: s.files_restored,
            bytes_restored: s.bytes_restored,
            dirs_created: s.dirs_created,
            renamed: s.renamed.iter().take(LIST_LIMIT).map(|(a, b)| [a.clone(), b.display().to_string()]).collect(),
            renamed_count: s.renamed.len(),
            skipped_links: cap(&s.skipped_links),
            skipped_links_count: s.skipped_links.len(),
            skipped_existing: cap(&s.skipped_existing),
            skipped_existing_count: s.skipped_existing.len(),
            errors: s.errors.iter().take(LIST_LIMIT).map(|(a, b)| [a.clone(), b.clone()]).collect(),
            errors_count: s.errors.len(),
        }
    }
}

/// What the UI shows for a job.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobDto {
    id: u64,
    /// `queued`, `running`, `done`, `cancelled` or `failed`.
    state: &'static str,
    /// `scanning`, `downloading` or `finishing` while running.
    phase: &'static str,
    source_path: String,
    snapshot_time: String,
    items: Vec<String>,
    item_count: usize,
    dest: String,
    dirs_scanned: u64,
    files_total: u64,
    files_done: u64,
    bytes_total: u64,
    bytes_done: u64,
    current: String,
    summary: Option<SummaryDto>,
    error: Option<String>,
}

struct Work {
    client: Arc<KopiaClient>,
    items: Vec<DirEntry>,
    dest: PathBuf,
    opts: RestoreOptions,
}

struct Job {
    dto: JobDto,
    cancel: Arc<AtomicBool>,
    work: Option<Work>,
}

pub struct RestoreManager {
    jobs: Mutex<Vec<Job>>,
    next_id: AtomicU64,
    queue: tokio::sync::mpsc::UnboundedSender<u64>,
}

impl RestoreManager {
    /// Creates the manager and starts the worker that runs queued jobs one by one.
    pub fn start(app: &AppHandle) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u64>();
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(id) = rx.recv().await {
                run_job(&handle, id).await;
            }
        });
        RestoreManager { jobs: Mutex::default(), next_id: AtomicU64::new(1), queue: tx }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Job>> {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn dto(&self, id: u64) -> Option<JobDto> {
        self.lock().iter().find(|j| j.dto.id == id).map(|j| j.dto.clone())
    }

    fn update(&self, id: u64, f: impl FnOnce(&mut Job)) -> Option<JobDto> {
        let mut jobs = self.lock();
        let job = jobs.iter_mut().find(|j| j.dto.id == id)?;
        f(job);
        Some(job.dto.clone())
    }

    /// Whether a restore is queued or running.
    pub fn busy(&self) -> bool {
        self.lock().iter().any(|j| matches!(j.dto.state, "queued" | "running"))
    }

    /// Cancels every queued or running job (used when disconnecting).
    pub fn cancel_all(&self) {
        for job in self.lock().iter_mut() {
            job.cancel.store(true, Ordering::Relaxed);
            if job.dto.state == "queued" {
                job.dto.state = "cancelled";
                job.work = None;
            }
        }
    }
}

fn emit(app: &AppHandle, dto: Option<JobDto>) {
    if let Some(dto) = dto {
        let _ = app.emit("restore-progress", dto);
    }
}

async fn run_job(app: &AppHandle, id: u64) {
    let manager = app.state::<RestoreManager>();
    let taken = {
        let mut jobs = manager.lock();
        jobs.iter_mut().find(|j| j.dto.id == id).and_then(|j| j.work.take().map(|w| (w, j.cancel.clone())))
    };
    let Some((work, cancel)) = taken else {
        return; // cancelled while queued
    };
    emit(app, manager.update(id, |j| j.dto.state = "running"));

    let last = Mutex::new(Instant::now());
    let on_progress = |p: &Progress| {
        let dto = manager.update(id, |j| {
            let d = &mut j.dto;
            d.phase = match p.phase {
                Phase::Scanning => "scanning",
                Phase::Downloading => "downloading",
                Phase::Finishing => "finishing",
            };
            d.dirs_scanned = p.dirs_scanned;
            d.files_total = p.files_total;
            d.files_done = p.files_done;
            d.bytes_total = p.bytes_total;
            d.bytes_done = p.bytes_done;
            d.current.clone_from(&p.current);
        });
        let mut l = last.lock().unwrap_or_else(|e| e.into_inner());
        if l.elapsed().as_millis() >= 150 {
            *l = Instant::now();
            emit(app, dto);
        }
    };

    let result = restore::restore_items(&work.client, &work.items, &work.dest, &work.opts, &cancel, &on_progress).await;
    let dto = manager.update(id, |j| match &result {
        Ok(summary) => {
            j.dto.state = if summary.cancelled { "cancelled" } else { "done" };
            j.dto.summary = Some(summary.into());
            j.dto.current.clear();
        }
        Err(e) => {
            j.dto.state = "failed";
            j.dto.error = Some(e.to_string());
        }
    });
    emit(app, dto);
}

// --- commands --------------------------------------------------------------------------------------

/// Asks the user for a destination folder (native dialog), starting in `start` if given.
#[tauri::command]
pub async fn choose_destination(
    app: AppHandle,
    start: Option<String>,
    title: String,
) -> Result<Option<String>, CmdError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app.dialog().file().set_title(title);
        if let Some(window) = app.get_webview_window("main") {
            dialog = dialog.set_parent(&window);
        }
        if let Some(dir) = start.filter(|s| std::path::Path::new(s).is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        dialog.blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
    })
    .await
    .map_err(|e| CmdError::other(e.to_string()))
}

fn naming(req: &RestoreRequest, snapshot_time: DateTime<Utc>) -> Naming {
    if req.naming == "original" {
        return Naming::Original;
    }
    let local = snapshot_time.with_timezone(&Local);
    let date = if req.suffix_date == "modified" {
        SuffixDate::Modified { fallback: local }
    } else {
        SuffixDate::Snapshot(local)
    };
    Naming::Suffix { label: req.label.clone(), date }
}

/// Resolves the request to the exact entries of the snapshot that was shown.
async fn resolve(client: &KopiaClient, req: &RestoreRequest) -> Result<(Vec<DirEntry>, DateTime<Utc>), CmdError> {
    let root: Oid = req.snapshot_root.parse()?;
    let time = DateTime::parse_from_rfc3339(&req.snapshot_time)
        .map_err(|e| CmdError::other(format!("invalid snapshot time: {e}")))?
        .with_timezone(&Utc);
    if req.names.is_empty() {
        // The shown folder itself.
        let Some((last, parent)) = req.path.split_last() else {
            return Ok((vec![restore::root_entry(&req.source.path, root, time)], time));
        };
        let r = client.resolve_path(&root, parent).await?;
        let entry = r.listing.entries.iter().find(|e| &e.name == last && e.entry_type == EntryType::Directory);
        return match (r.depth == parent.len(), entry) {
            (true, Some(e)) => Ok((vec![e.clone()], time)),
            _ => Err(CmdError::new("not_found", "folder not in this snapshot")),
        };
    }
    let r = client.resolve_path(&root, &req.path).await?;
    if r.depth != req.path.len() {
        return Err(CmdError::new("not_found", "folder not in this snapshot"));
    }
    let mut items = Vec::with_capacity(req.names.len());
    for name in &req.names {
        match r.listing.entries.iter().find(|e| &e.name == name) {
            Some(e) => items.push(e.clone()),
            None => return Err(CmdError::new("not_found", format!("{name} not in this snapshot"))),
        }
    }
    Ok((items, time))
}

/// Top-level targets that already exist in the destination (asked before overwriting).
#[tauri::command]
pub async fn restore_conflicts(
    state: tauri::State<'_, AppState>,
    request: RestoreRequest,
) -> Result<Vec<String>, CmdError> {
    let client = state.client()?;
    let (items, time) = resolve(&client, &request).await?;
    let dest = PathBuf::from(&request.dest);
    Ok(restore::existing_targets(&items, &dest, &naming(&request, time))
        .iter()
        .map(|p| p.display().to_string())
        .collect())
}

#[tauri::command]
pub async fn start_restore(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    manager: tauri::State<'_, RestoreManager>,
    request: RestoreRequest,
) -> Result<u64, CmdError> {
    let client = state.client()?;
    let conflict = match request.conflict.as_str() {
        "skip" => ConflictPolicy::Skip,
        "overwrite" if request.overwrite_confirmed => ConflictPolicy::Overwrite { confirmed: true },
        "overwrite" => return Err(CmdError::other("overwriting was not confirmed")),
        _ => ConflictPolicy::Rename,
    };
    let dest = PathBuf::from(&request.dest);
    if !dest.is_absolute() || !dest.is_dir() {
        return Err(CmdError::other(format!("destination folder {} does not exist", dest.display())));
    }
    let (items, time) = resolve(&client, &request).await?;
    let mut opts = RestoreOptions::new(naming(&request, time), conflict);
    opts.verify = request.verify;

    let id = manager.next_id.fetch_add(1, Ordering::Relaxed);
    let dto = JobDto {
        id,
        state: "queued",
        phase: "scanning",
        source_path: request.source.path.clone(),
        snapshot_time: request.snapshot_time.clone(),
        items: items.iter().take(5).map(|e| e.name.clone()).collect(),
        item_count: items.len(),
        dest: dest.display().to_string(),
        dirs_scanned: 0,
        files_total: 0,
        files_done: 0,
        bytes_total: 0,
        bytes_done: 0,
        current: String::new(),
        summary: None,
        error: None,
    };
    manager.lock().push(Job {
        dto: dto.clone(),
        cancel: Arc::new(AtomicBool::new(false)),
        work: Some(Work { client, items, dest, opts }),
    });
    emit(&app, Some(dto));
    manager.queue.send(id).map_err(|_| CmdError::other("restore queue stopped"))?;
    Ok(id)
}

#[tauri::command]
pub fn list_restores(manager: tauri::State<'_, RestoreManager>) -> Vec<JobDto> {
    manager.lock().iter().map(|j| j.dto.clone()).collect()
}

#[tauri::command]
pub fn cancel_restore(app: AppHandle, manager: tauri::State<'_, RestoreManager>, id: u64) {
    let dto = manager.update(id, |j| {
        j.cancel.store(true, Ordering::Relaxed);
        if j.dto.state == "queued" {
            j.dto.state = "cancelled";
            j.work = None;
        }
    });
    emit(&app, dto);
}

/// Removes finished jobs from the list.
#[tauri::command]
pub fn clear_restores(manager: tauri::State<'_, RestoreManager>) {
    manager.lock().retain(|j| matches!(j.dto.state, "queued" | "running"));
}

/// Shows the restored items of job `id` in the system file manager. Only paths of that job are
/// ever opened; nothing from the UI is passed to the shell.
#[tauri::command]
pub fn open_restore_folder(manager: tauri::State<'_, RestoreManager>, id: u64) -> Result<(), CmdError> {
    let Some(dto) = manager.dto(id) else { return Err(CmdError::other("unknown restore")) };
    let first = dto.summary.as_ref().and_then(|s| s.top_level.first().cloned());
    let target = first.map(PathBuf::from).filter(|p| p.exists());
    let dest = PathBuf::from(&dto.dest);
    let spawn = |cmd: &mut std::process::Command| cmd.spawn().map(|_| ()).map_err(|e| CmdError::other(e.to_string()));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("explorer.exe");
        // Explorer parses its own command line and expects exactly `/select,"C:\path with spaces"`.
        // The path comes from this job's own result; it cannot contain `"` (invalid on Windows).
        match &target {
            Some(p) => cmd.raw_arg(format!("/select,\"{}\"", p.display())),
            None => cmd.raw_arg(format!("\"{}\"", dest.display())),
        };
        spawn(&mut cmd)
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        match &target {
            Some(p) => cmd.arg("-R").arg(p),
            None => cmd.arg(&dest),
        };
        spawn(&mut cmd)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = target;
        spawn(std::process::Command::new("xdg-open").arg(&dest))
    }
}
