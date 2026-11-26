//! Restoring files and folders to the local disk.
//!
//! Rules (see `docs/API.md` §6.4–§6.5 and the project principles):
//! * Data is downloaded through the API and written locally; nothing is restored on the server.
//! * Names coming from the server are untrusted: every component is sanitized, `.`/`..`/empty
//!   names are neutralized and every target is checked to stay inside the destination folder.
//! * By default nothing is overwritten. Overwriting requires [`ConflictPolicy::Overwrite`] with
//!   `confirmed: true`, which callers only set after asking the user; without it the restore is
//!   refused. Existing symbolic links or junctions are never written through.
//! * Every file is streamed into a temporary `.rewind-*.part` file in the target folder and only
//!   renamed onto its final name once the size matches. Modification times are restored
//!   (folders last, deepest first) and optionally verified.
//! * Symbolic links are not restored; they are reported.
//! * Only the top-level items get the "(restored …)" suffix; folder contents keep their names.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local, Utc};
use futures_util::StreamExt;
use futures_util::stream;
use tokio::io::AsyncWriteExt;

use crate::{DirEntry, EntryType, Error, KopiaClient, Oid, Result};

/// Maximum length of one path component in bytes (255 on all common file systems).
const MAX_NAME_BYTES: usize = 255;
/// FAT32 stores modification times in 2-second steps.
const MTIME_TOLERANCE: Duration = Duration::from_secs(2);
/// Whether names that differ only in case collide on this platform.
const CASE_INSENSITIVE: bool = cfg!(any(windows, target_os = "macos"));

/// Which date goes into the "(version of …)" suffix.
#[derive(Debug, Clone)]
pub enum SuffixDate {
    /// The snapshot's start time (local time).
    Snapshot(DateTime<Local>),
    /// Each item's own modification time; `fallback` when it is unknown.
    Modified { fallback: DateTime<Local> },
}

/// How to name the restored top-level items.
#[derive(Debug, Clone)]
pub enum Naming {
    /// `name (<label> YYYY-MM-DD HHhmm).ext`. `label` is user-interface text, already translated and
    /// matching the date (e.g. "version of" for the snapshot time, "modified" for the file date).
    Suffix { label: String, date: SuffixDate },
    /// Keep the original name.
    Original,
}

/// What to do when a target already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictPolicy {
    /// Keep both: the restored item gets `name (2)`, `name (3)`, … (default).
    Rename,
    /// Leave existing files alone; restore only what is missing (merges into existing folders).
    Skip,
    /// Replace existing files (merges into existing folders). `confirmed` must be true: set it
    /// only after the user explicitly agreed to overwrite.
    Overwrite { confirmed: bool },
}

#[derive(Debug, Clone)]
pub struct RestoreOptions {
    pub naming: Naming,
    pub conflict: ConflictPolicy,
    /// Re-read size and modification time of every restored file.
    pub verify: bool,
    /// Files downloaded at the same time.
    pub file_concurrency: usize,
    /// Folders listed at the same time while scanning.
    pub dir_concurrency: usize,
}

impl RestoreOptions {
    pub fn new(naming: Naming, conflict: ConflictPolicy) -> Self {
        RestoreOptions { naming, conflict, verify: true, file_concurrency: 4, dir_concurrency: 4 }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Phase {
    /// Listing folders and creating the folder structure.
    #[default]
    Scanning,
    /// Downloading files.
    Downloading,
    /// Setting folder dates.
    Finishing,
}

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub phase: Phase,
    pub dirs_scanned: u64,
    pub files_total: u64,
    pub files_done: u64,
    pub bytes_total: u64,
    pub bytes_done: u64,
    /// Snapshot path of the item being processed.
    pub current: String,
}

/// Result of a restore. Paths given as `String` are paths inside the snapshot.
#[derive(Debug, Clone, Default)]
pub struct Summary {
    /// Where the selected items were written.
    pub top_level: Vec<PathBuf>,
    pub files_restored: u64,
    pub bytes_restored: u64,
    pub dirs_created: u64,
    /// Items whose name had to change (invalid characters, collisions, length).
    pub renamed: Vec<(String, PathBuf)>,
    pub skipped_links: Vec<String>,
    pub skipped_existing: Vec<String>,
    pub errors: Vec<(String, String)>,
    pub cancelled: bool,
}

/// Makes `name` a valid Windows file name: invalid characters (`< > : " / \ | ? *` and control
/// characters) become `_`; reserved device names (`CON`, `NUL`, `COM1`, ...) and names ending in
/// a dot or a space get `_` appended to the base name; empty, `.` and `..` become `_`.
pub fn sanitize_windows_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| if c < ' ' || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') { '_' } else { c })
        .collect();
    if out.is_empty() || out == "." || out == ".." {
        return "_".into();
    }
    if out.ends_with('.') || out.ends_with(' ') {
        out.push('_');
    }
    let base_len = out.find('.').unwrap_or(out.len());
    let base = out[..base_len].trim_end().to_ascii_uppercase();
    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.as_bytes()[3].is_ascii_digit()
            && base.as_bytes()[3] != b'0');
    if reserved {
        out.insert(base_len, '_');
    }
    out
}

/// A safe single path component for this platform, at most 255 bytes.
pub fn safe_name(name: &str) -> String {
    let s = if cfg!(windows) {
        sanitize_windows_name(name)
    } else {
        let s: String = name.chars().map(|c| if c == '/' || c == '\0' { '_' } else { c }).collect();
        if s.is_empty() || s == "." || s == ".." { "_".into() } else { s }
    };
    if s.len() <= MAX_NAME_BYTES {
        s
    } else {
        let (stem, ext) = split_ext(&s);
        compose(stem, "", ext)
    }
}

/// Splits `name` into base and extension (`.ext`, including the dot). Names starting with a dot
/// and without another dot have no extension. Long "extensions" are not treated as such.
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && name.len() - i <= 16 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

fn split_for(name: &str, is_dir: bool) -> (&str, &str) {
    if is_dir { (name, "") } else { split_ext(name) }
}

fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut i = max;
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    &s[..i]
}

/// `stem + middle + ext`, shortening the stem so the result fits in 255 bytes.
fn compose(stem: &str, middle: &str, ext: &str) -> String {
    let room = MAX_NAME_BYTES.saturating_sub(middle.len() + ext.len());
    let mut stem = truncate_bytes(stem, room).trim_end_matches([' ', '.']).to_owned();
    if stem.is_empty() {
        stem.push('_');
    }
    format!("{stem}{middle}{ext}")
}

/// Name of a restored top-level item (before collision handling).
pub fn restored_name(name: &str, is_dir: bool, mtime: Option<DateTime<Utc>>, naming: &Naming) -> String {
    match naming {
        Naming::Original => name.to_owned(),
        Naming::Suffix { label, date } => {
            let t = match date {
                SuffixDate::Snapshot(t) => *t,
                SuffixDate::Modified { fallback } => mtime.map(|m| m.with_timezone(&Local)).unwrap_or(*fallback),
            };
            let (stem, ext) = split_for(name, is_dir);
            compose(stem, &format!(" ({label} {})", t.format("%Y-%m-%d %Hh%M")), ext)
        }
    }
}

fn with_counter(name: &str, is_dir: bool, n: u32) -> String {
    if n <= 1 {
        return name.to_owned();
    }
    let (stem, ext) = split_for(name, is_dir);
    compose(stem, &format!(" ({n})"), ext)
}

/// Names already used inside one folder being restored (case-insensitive where the OS is).
#[derive(Default)]
struct NameSet(HashSet<String>);

impl NameSet {
    fn key(n: &str) -> String {
        if CASE_INSENSITIVE { n.to_lowercase() } else { n.to_owned() }
    }

    fn claim(&mut self, name: &str, is_dir: bool) -> String {
        let mut n = 1;
        loop {
            let candidate = with_counter(name, is_dir, n);
            if self.0.insert(Self::key(&candidate)) {
                return candidate;
            }
            n += 1;
        }
    }
}

/// `parent/name`, refusing anything that would leave `dest` or is not a plain component.
fn checked_join(dest: &Path, parent: &Path, name: &str) -> Result<PathBuf> {
    let p = parent.join(name);
    let ok = p
        .strip_prefix(dest)
        .map(|rel| rel.components().count() > 0 && rel.components().all(|c| matches!(c, Component::Normal(_))))
        .unwrap_or(false)
        && Path::new(name).components().count() == 1;
    if ok { Ok(p) } else { Err(Error::Other(format!("unsafe file name {name:?}"))) }
}

#[derive(Debug, PartialEq, Eq)]
enum Existing {
    None,
    File,
    Dir,
    /// Symbolic link, junction or anything else that must not be written through.
    Other,
}

async fn existing(p: &Path) -> Existing {
    match tokio::fs::symlink_metadata(p).await {
        Err(_) => Existing::None,
        Ok(m) if m.file_type().is_symlink() => Existing::Other,
        Ok(m) if m.is_dir() => Existing::Dir,
        Ok(m) if m.is_file() => Existing::File,
        Ok(_) => Existing::Other,
    }
}

/// Top-level targets of `items` in `dest` that already exist (to ask before overwriting).
pub fn existing_targets(items: &[DirEntry], dest: &Path, naming: &Naming) -> Vec<PathBuf> {
    items
        .iter()
        .filter(|e| matches!(e.entry_type, EntryType::File | EntryType::Directory))
        .filter_map(|e| {
            let is_dir = e.entry_type == EntryType::Directory;
            let name = restored_name(&safe_name(&e.name), is_dir, e.mtime, naming);
            let p = checked_join(dest, dest, &name).ok()?;
            std::fs::symlink_metadata(&p).is_ok().then_some(p)
        })
        .collect()
}

/// Entry representing a whole snapshot (its root folder), named after the source folder.
pub fn root_entry(source_path: &str, root: Oid, snapshot_time: DateTime<Utc>) -> DirEntry {
    let name = source_path.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("");
    DirEntry {
        name: if name.is_empty() { "root".into() } else { name.to_owned() },
        entry_type: EntryType::Directory,
        mode: None,
        size: 0,
        mtime: Some(snapshot_time),
        uid: 0,
        gid: 0,
        obj: root,
        summ: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteMode {
    /// An empty placeholder with the final name was already created by this restore.
    Placeholder,
    /// The final name must not exist; it is created when the download starts.
    CreateNew,
    /// Replace an existing regular file (confirmed overwrite only).
    Replace,
}

struct FileTask {
    entry: DirEntry,
    target: PathBuf,
    display: String,
    mode: WriteMode,
}

struct DirJob {
    oid: Oid,
    local: PathBuf,
    display: String,
    /// Created by this restore (empty), as opposed to an existing folder being merged into.
    fresh: bool,
    depth: usize,
}

struct Shared<'a> {
    progress: Mutex<Progress>,
    summary: Mutex<Summary>,
    callback: &'a (dyn Fn(&Progress) + Send + Sync),
}

impl Shared<'_> {
    fn update(&self, f: impl FnOnce(&mut Progress)) {
        let snapshot = {
            let mut p = self.progress.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut p);
            p.clone()
        };
        (self.callback)(&snapshot);
    }

    fn summary(&self) -> std::sync::MutexGuard<'_, Summary> {
        self.summary.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Restores `items` (entries of one snapshot folder, or [`root_entry`]) into `dest`.
///
/// Errors on individual files are collected in [`Summary::errors`] and do not stop the restore.
/// Returns an error only if the restore cannot start (invalid destination, unconfirmed
/// overwrite). When `cancel` becomes true, running downloads stop, their temporary files are
/// removed and files already restored are kept.
pub async fn restore_items(
    client: &KopiaClient,
    items: &[DirEntry],
    dest: &Path,
    opts: &RestoreOptions,
    cancel: &AtomicBool,
    progress: &(dyn Fn(&Progress) + Send + Sync),
) -> Result<Summary> {
    if opts.conflict == (ConflictPolicy::Overwrite { confirmed: false }) {
        return Err(Error::Other("overwriting requires explicit confirmation".into()));
    }
    match tokio::fs::metadata(dest).await {
        Ok(m) if m.is_dir() => {}
        _ => return Err(Error::Other(format!("destination folder {} does not exist", dest.display()))),
    }

    let shared = Shared { progress: Mutex::default(), summary: Mutex::default(), callback: progress };
    let mut tasks: Vec<FileTask> = Vec::new();
    let mut dirs: Vec<DirJob> = Vec::new();
    let mut dir_dates: Vec<(PathBuf, usize, DateTime<Utc>)> = Vec::new();

    // --- top-level items ---
    let mut top_names = NameSet::default();
    for item in items {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let display = item.name.clone();
        let is_dir = match item.entry_type {
            EntryType::File => false,
            EntryType::Directory => true,
            _ => {
                shared.summary().skipped_links.push(display);
                continue;
            }
        };
        let sanitized = safe_name(&item.name);
        let base = top_names.claim(&restored_name(&sanitized, is_dir, item.mtime, &opts.naming), is_dir);
        let planned = plan_top(item, is_dir, dest, &base, opts.conflict).await;
        let (target, outcome) = match planned {
            Ok(x) => x,
            Err(e) => {
                shared.summary().errors.push((display, e.to_string()));
                continue;
            }
        };
        if sanitized != item.name {
            shared.summary().renamed.push((display.clone(), target.clone()));
        }
        match outcome {
            TopOutcome::SkipExisting => {
                shared.summary().skipped_existing.push(display);
                continue;
            }
            TopOutcome::File(mode) => {
                shared.update(|p| {
                    p.files_total += 1;
                    p.bytes_total += item.size;
                });
                tasks.push(FileTask { entry: item.clone(), target: target.clone(), display, mode });
            }
            TopOutcome::Dir { fresh } => {
                if fresh {
                    shared.summary().dirs_created += 1;
                    if let Some(m) = item.mtime {
                        dir_dates.push((target.clone(), 0, m));
                    }
                }
                dirs.push(DirJob { oid: item.obj.clone(), local: target.clone(), display, fresh, depth: 0 });
            }
        }
        shared.summary().top_level.push(target);
    }

    // --- scan folders level by level, listing a few at a time ---
    while !dirs.is_empty() && !cancel.load(Ordering::Relaxed) {
        let listings: Vec<_> = stream::iter(dirs)
            .map(|job| async move {
                let r = client.list_directory(&job.oid).await;
                (job, r)
            })
            .buffer_unordered(opts.dir_concurrency.max(1))
            .collect()
            .await;
        let mut next = Vec::new();
        for (job, listing) in listings {
            shared.update(|p| {
                p.dirs_scanned += 1;
                p.current = job.display.clone();
            });
            match listing {
                Ok(m) => {
                    plan_children(&job, &m.entries, dest, opts.conflict, &shared, &mut tasks, &mut next, &mut dir_dates)
                        .await
                }
                Err(e) => shared.summary().errors.push((job.display.clone(), e.to_string())),
            }
        }
        dirs = next;
    }

    // --- download files ---
    shared.update(|p| p.phase = Phase::Downloading);
    stream::iter(tasks)
        .for_each_concurrent(opts.file_concurrency.max(1), |task| {
            let shared = &shared;
            async move {
                if cancel.load(Ordering::Relaxed) {
                    if task.mode == WriteMode::Placeholder {
                        remove_placeholder(&task.target).await;
                    }
                    return;
                }
                shared.update(|p| p.current = task.display.clone());
                let written = AtomicU64::new(0);
                let on_bytes = |n: u64| {
                    written.fetch_add(n, Ordering::Relaxed);
                    shared.update(|p| p.bytes_done += n);
                };
                let result = download(client, &task, opts.verify, cancel, &on_bytes).await;
                let partial = written.load(Ordering::Relaxed);
                match result {
                    Ok(bytes) => {
                        let mut s = shared.summary();
                        s.files_restored += 1;
                        s.bytes_restored += bytes;
                        drop(s);
                        shared.update(|p| p.files_done += 1);
                    }
                    Err(Error::Cancelled) => {}
                    Err(e) => {
                        shared.summary().errors.push((task.display.clone(), e.to_string()));
                        // Count the file as processed so the progress still reaches 100 %.
                        shared.update(|p| {
                            p.files_done += 1;
                            p.bytes_done = p.bytes_done - partial.min(p.bytes_done) + task.entry.size;
                        });
                    }
                }
            }
        })
        .await;

    // --- folder dates, deepest first (adding files changes them) ---
    shared.update(|p| p.phase = Phase::Finishing);
    dir_dates.sort_by_key(|d| std::cmp::Reverse(d.1));
    for (path, _, mtime) in dir_dates {
        if let Err(e) = set_mtime(&path, mtime, true).await {
            shared.summary().errors.push((path.display().to_string(), e.to_string()));
        }
    }

    let mut summary = shared.summary.into_inner().unwrap_or_else(|e| e.into_inner());
    summary.cancelled = cancel.load(Ordering::Relaxed);
    Ok(summary)
}

enum TopOutcome {
    File(WriteMode),
    Dir { fresh: bool },
    SkipExisting,
}

async fn plan_top(
    item: &DirEntry,
    is_dir: bool,
    dest: &Path,
    base: &str,
    policy: ConflictPolicy,
) -> Result<(PathBuf, TopOutcome)> {
    if policy == ConflictPolicy::Rename {
        // Keep both: find a free name, reserving it atomically.
        for n in 1..=9999u32 {
            let path = checked_join(dest, dest, &with_counter(base, is_dir, n))?;
            let created = if is_dir {
                tokio::fs::create_dir(&path).await
            } else {
                tokio::fs::OpenOptions::new().write(true).create_new(true).open(&path).await.map(|_| ())
            };
            match created {
                Ok(()) => {
                    let outcome =
                        if is_dir { TopOutcome::Dir { fresh: true } } else { TopOutcome::File(WriteMode::Placeholder) };
                    return Ok((path, outcome));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(Error::io(format!("cannot create {}", path.display()), e)),
            }
        }
        return Err(Error::Other(format!("no free name for {}", item.name)));
    }

    let target = checked_join(dest, dest, base)?;
    let outcome = match (existing(&target).await, is_dir) {
        (Existing::None, true) => {
            tokio::fs::create_dir(&target)
                .await
                .map_err(|e| Error::io(format!("cannot create {}", target.display()), e))?;
            TopOutcome::Dir { fresh: true }
        }
        (Existing::None, false) => TopOutcome::File(WriteMode::CreateNew),
        (Existing::Dir, true) => TopOutcome::Dir { fresh: false },
        (Existing::File, false) if policy == ConflictPolicy::Skip => TopOutcome::SkipExisting,
        (Existing::File, false) => TopOutcome::File(WriteMode::Replace),
        (Existing::Other, _) => {
            return Err(Error::Other(format!("{} exists and is a link; not touched", target.display())));
        }
        _ => return Err(Error::Other(format!("{} exists with a different type; not touched", target.display()))),
    };
    Ok((target, outcome))
}

#[allow(clippy::too_many_arguments)]
async fn plan_children(
    job: &DirJob,
    entries: &[DirEntry],
    dest: &Path,
    policy: ConflictPolicy,
    shared: &Shared<'_>,
    tasks: &mut Vec<FileTask>,
    next: &mut Vec<DirJob>,
    dir_dates: &mut Vec<(PathBuf, usize, DateTime<Utc>)>,
) {
    let mut names = NameSet::default();
    for e in entries {
        let display = format!("{}/{}", job.display, e.name);
        let is_dir = match e.entry_type {
            EntryType::File => false,
            EntryType::Directory => true,
            _ => {
                shared.summary().skipped_links.push(display);
                continue;
            }
        };
        let name = names.claim(&safe_name(&e.name), is_dir);
        let target = match checked_join(dest, &job.local, &name) {
            Ok(p) => p,
            Err(err) => {
                shared.summary().errors.push((display, err.to_string()));
                continue;
            }
        };
        if name != e.name {
            shared.summary().renamed.push((display.clone(), target.clone()));
        }
        let state = if job.fresh { Existing::None } else { existing(&target).await };
        if is_dir {
            let fresh = match state {
                Existing::None => match tokio::fs::create_dir(&target).await {
                    Ok(()) => true,
                    Err(err) => {
                        shared.summary().errors.push((display, format!("cannot create folder: {err}")));
                        continue;
                    }
                },
                Existing::Dir => false,
                _ => {
                    shared.summary().errors.push((display, "exists and is not a folder; not touched".into()));
                    continue;
                }
            };
            if fresh {
                shared.summary().dirs_created += 1;
                if let Some(m) = e.mtime {
                    dir_dates.push((target.clone(), job.depth + 1, m));
                }
            }
            next.push(DirJob { oid: e.obj.clone(), local: target, display, fresh, depth: job.depth + 1 });
        } else {
            let mode = match state {
                Existing::None => WriteMode::CreateNew,
                Existing::File if matches!(policy, ConflictPolicy::Overwrite { confirmed: true }) => WriteMode::Replace,
                Existing::File => {
                    shared.summary().skipped_existing.push(display);
                    continue;
                }
                _ => {
                    shared.summary().errors.push((display, "exists and is not a regular file; not touched".into()));
                    continue;
                }
            };
            shared.update(|p| {
                p.files_total += 1;
                p.bytes_total += e.size;
            });
            tasks.push(FileTask { entry: e.clone(), target, display, mode });
        }
    }
}

/// Removes an empty placeholder created by this restore.
async fn remove_placeholder(path: &Path) {
    if let Ok(m) = tokio::fs::symlink_metadata(path).await
        && m.is_file()
        && m.len() == 0
    {
        let _ = tokio::fs::remove_file(path).await;
    }
}

fn part_path(target: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    target.with_file_name(format!(".rewind-{:x}-{n:x}.part", std::process::id()))
}

async fn download(
    client: &KopiaClient,
    task: &FileTask,
    verify: bool,
    cancel: &AtomicBool,
    on_bytes: &(dyn Fn(u64) + Sync),
) -> Result<u64> {
    let target = &task.target;
    let mut owns_placeholder = task.mode == WriteMode::Placeholder;
    if task.mode == WriteMode::CreateNew {
        tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
            .await
            .map_err(|e| Error::io(format!("cannot create {}", target.display()), e))?;
        owns_placeholder = true;
    }
    let part = part_path(target);

    // Everything up to the rename can be undone completely.
    let staged = async {
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)
            .await
            .map_err(|e| Error::io(format!("cannot create {}", part.display()), e))?;
        let mut out = tokio::io::BufWriter::with_capacity(1 << 20, file);
        let mut stream = client.open_object(&task.entry.obj, None).await?;
        let mut written: u64 = 0;
        while let Some(chunk) = stream.chunk().await? {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            written += chunk.len() as u64;
            out.write_all(&chunk).await.map_err(|e| Error::io(format!("cannot write {}", part.display()), e))?;
            on_bytes(chunk.len() as u64);
        }
        out.flush().await.map_err(|e| Error::io(format!("cannot write {}", part.display()), e))?;
        let file = out.into_inner();
        file.sync_all().await.map_err(|e| Error::io(format!("cannot write {}", part.display()), e))?;
        drop(file);
        if written != task.entry.size {
            return Err(Error::SizeMismatch { path: task.display.clone(), expected: task.entry.size, actual: written });
        }
        if task.mode == WriteMode::Replace && existing(target).await != Existing::File {
            return Err(Error::Other(format!("{} changed while restoring; not touched", target.display())));
        }
        tokio::fs::rename(&part, target)
            .await
            .map_err(|e| Error::io(format!("cannot rename to {}", target.display()), e))?;
        Ok(written)
    }
    .await;

    let written = match staged {
        Ok(w) => w,
        Err(e) => {
            let _ = tokio::fs::remove_file(&part).await;
            if owns_placeholder {
                remove_placeholder(target).await;
            }
            return Err(e);
        }
    };

    // The file is in place; later problems are reported but the file is kept.
    if let Some(mtime) = task.entry.mtime {
        set_mtime(target, mtime, false).await?;
    }
    if verify {
        let m = tokio::fs::metadata(target).await.map_err(|e| Error::io("cannot verify", e))?;
        if m.len() != task.entry.size {
            return Err(Error::SizeMismatch { path: task.display.clone(), expected: task.entry.size, actual: m.len() });
        }
        if let (Some(expected), Ok(actual)) = (task.entry.mtime, m.modified()) {
            let expected = SystemTime::from(expected);
            let diff = actual.duration_since(expected).or_else(|_| expected.duration_since(actual)).unwrap_or_default();
            if diff > MTIME_TOLERANCE {
                return Err(Error::Other("verification failed: modification time differs".into()));
            }
        }
    }
    Ok(written)
}

async fn set_mtime(path: &Path, mtime: DateTime<Utc>, is_dir: bool) -> Result<()> {
    let path = path.to_path_buf();
    let modified = SystemTime::from(mtime);
    tokio::task::spawn_blocking(move || {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(!is_dir || cfg!(windows));
        #[cfg(windows)]
        if is_dir {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
            opts.custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
        }
        #[cfg(not(windows))]
        if is_dir {
            opts.read(true);
        }
        let f = opts.open(&path)?;
        f.set_times(std::fs::FileTimes::new().set_modified(modified))
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?
    .map_err(|e| Error::io("cannot set modification time", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn sanitizes_windows_names() {
        let cases = [
            ("normal.txt", "normal.txt"),
            ("a:b?c*.txt", "a_b_c_.txt"),
            ("back\\slash|pipe\"<>", "back_slash_pipe___"),
            ("tab\there", "tab_here"),
            ("CON", "CON_"),
            ("con.txt", "con_.txt"),
            ("Com1.tar.gz", "Com1_.tar.gz"),
            ("COM0", "COM0"),
            ("LPT9", "LPT9_"),
            ("CONSOLE.txt", "CONSOLE.txt"),
            ("ends with dot.", "ends with dot._"),
            ("ends with space ", "ends with space _"),
            (".hidden", ".hidden"),
            ("", "_"),
            (".", "_"),
            ("..", "_"),
            ("ñandú 日本.jpg", "ñandú 日本.jpg"),
        ];
        for (input, expected) in cases {
            assert_eq!(sanitize_windows_name(input), expected, "{input:?}");
        }
    }

    #[test]
    fn safe_names_never_escape_or_exceed_255_bytes() {
        for hostile in ["", ".", "..", "a/b", "../x", "a\0b"] {
            let s = safe_name(hostile);
            assert!(
                !s.is_empty() && s != "." && s != ".." && !s.contains('/') && !s.contains('\0'),
                "{hostile:?} -> {s:?}"
            );
        }
        let long = format!("{}.txt", "ñ".repeat(200));
        let s = safe_name(&long);
        assert!(s.len() <= 255 && s.ends_with(".txt"), "{} bytes", s.len());
    }

    #[test]
    fn builds_restored_names() {
        let time = Local.with_ymd_and_hms(2026, 11, 25, 19, 0, 0).unwrap();
        let naming = Naming::Suffix { label: "version of".into(), date: SuffixDate::Snapshot(time) };
        assert_eq!(restored_name("report.pdf", false, None, &naming), "report (version of 2026-11-25 19h00).pdf");
        assert_eq!(
            restored_name("archive.tar.gz", false, None, &naming),
            "archive.tar (version of 2026-11-25 19h00).gz"
        );
        assert_eq!(restored_name(".bashrc", false, None, &naming), ".bashrc (version of 2026-11-25 19h00)");
        assert_eq!(restored_name("v1.2", true, None, &naming), "v1.2 (version of 2026-11-25 19h00)");
        assert_eq!(restored_name("x.pdf", false, None, &Naming::Original), "x.pdf");

        let mtime = Local.with_ymd_and_hms(2025, 3, 4, 5, 6, 0).unwrap().with_timezone(&Utc);
        let by_file = Naming::Suffix { label: "modified".into(), date: SuffixDate::Modified { fallback: time } };
        assert_eq!(restored_name("a.txt", false, Some(mtime), &by_file), "a (modified 2025-03-04 05h06).txt");
        assert_eq!(restored_name("a.txt", false, None, &by_file), "a (modified 2026-11-25 19h00).txt");

        let long = format!("{}.pdf", "x".repeat(250));
        let n = restored_name(&long, false, None, &naming);
        assert!(n.len() <= 255 && n.ends_with(" (version of 2026-11-25 19h00).pdf"), "{n}");
        assert!(with_counter(&long, false, 12).len() <= 255);
    }

    #[test]
    fn name_set_resolves_collisions() {
        let mut s = NameSet::default();
        assert_eq!(s.claim("a.txt", false), "a.txt");
        assert_eq!(s.claim("a.txt", false), "a (2).txt");
        let upper = s.claim("A.txt", false);
        if CASE_INSENSITIVE {
            assert_eq!(upper, "A (3).txt");
        } else {
            assert_eq!(upper, "A.txt");
        }
    }

    #[test]
    fn checked_join_refuses_escapes() {
        let dest = Path::new("/restore");
        assert!(checked_join(dest, dest, "ok").is_ok());
        assert!(checked_join(dest, dest, "..").is_err());
        assert!(checked_join(dest, dest, ".").is_err());
        assert!(checked_join(dest, dest, "").is_err());
        assert!(checked_join(dest, dest, "a/b").is_err());
        assert!(checked_join(dest, Path::new("/elsewhere"), "x").is_err());
    }

    #[test]
    fn root_entry_names() {
        let oid: Oid = "k01".parse().unwrap();
        let t = Utc::now();
        assert_eq!(root_entry("/data/docs", oid.clone(), t).name, "docs");
        assert_eq!(root_entry("/data/docs/", oid.clone(), t).name, "docs");
        assert_eq!(root_entry("/", oid.clone(), t).name, "root");
        assert_eq!(root_entry("C:\\Users\\ana", oid, t).name, "ana");
    }
}
