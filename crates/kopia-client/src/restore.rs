//! Restoring files to the local disk.
//!
//! Rules (see `docs/API.md` §6.4–§6.5):
//! * Data is downloaded through the API and written locally; nothing is restored on the server.
//! * Existing files are never overwritten: a free name is reserved first (`name (2).ext`, ...).
//! * Data is streamed into a temporary `.rewind-part` file next to the target and only renamed
//!   onto the reserved name once the size matches the snapshot entry. The modification time is
//!   then set from the snapshot.
//! * On Windows, invalid file names are made valid by replacing characters with `_`.
//! * Symbolic links are not restored.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use tokio::io::AsyncWriteExt;

use crate::{DirEntry, EntryType, Error, KopiaClient, Result};

/// How to name the restored copy.
#[derive(Debug, Clone)]
pub enum Naming {
    /// `name (<label> YYYY-MM-DD HH-mm).ext`, e.g. `report (restored 2026-11-25 19-00).pdf`.
    /// `label` is user-interface text (translated by the caller); the time is normally the
    /// snapshot's start time in local time.
    Suffix { label: String, time: DateTime<Local> },
    /// Keep the original name (still never overwriting: `name (2).ext` if taken).
    Original,
}

/// Outcome of [`restore_file`].
#[derive(Debug, Clone)]
pub struct RestoredFile {
    pub path: PathBuf,
    pub bytes: u64,
    /// The name had to be changed to be valid on this system.
    pub sanitized: bool,
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

/// Splits `name` into base and extension (`.ext`, including the dot). Names starting with a dot
/// and without another dot have no extension.
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

/// Name of the restored copy for `name` according to `naming` (before collision handling).
pub fn restored_name(name: &str, naming: &Naming) -> String {
    match naming {
        Naming::Original => name.to_owned(),
        Naming::Suffix { label, time } => {
            let (base, ext) = split_ext(name);
            format!("{base} ({label} {}){ext}", time.format("%Y-%m-%d %H-%M"))
        }
    }
}

/// Atomically reserves a free file name in `dir` starting from `name`, by creating an empty file.
/// Returns the reserved path.
async fn reserve(dir: &Path, name: &str) -> Result<PathBuf> {
    let (base, ext) = split_ext(name);
    for n in 1..=9999u32 {
        let candidate = if n == 1 { name.to_owned() } else { format!("{base} ({n}){ext}") };
        let path = dir.join(&candidate);
        match tokio::fs::OpenOptions::new().write(true).create_new(true).open(&path).await {
            Ok(_) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(Error::io(format!("cannot create {}", path.display()), e)),
        }
    }
    Err(Error::Other(format!("no free file name for {name} in {}", dir.display())))
}

/// Downloads the file `entry` into `dest_dir`.
///
/// Fails for directories (walk them and call this per file) and for symbolic links, which are
/// not restored.
pub async fn restore_file(
    client: &KopiaClient,
    entry: &DirEntry,
    dest_dir: &Path,
    naming: &Naming,
) -> Result<RestoredFile> {
    match entry.entry_type {
        EntryType::File => {}
        EntryType::Symlink => {
            return Err(Error::Other(format!("{} is a symbolic link; links are not restored", entry.name)));
        }
        _ => return Err(Error::Other(format!("{} is not a file", entry.name))),
    }

    let valid_name = if cfg!(windows) { sanitize_windows_name(&entry.name) } else { entry.name.replace('/', "_") };
    let sanitized = valid_name != entry.name;
    let target = reserve(dest_dir, &restored_name(&valid_name, naming)).await?;
    let mut part_name = target.file_name().unwrap_or_default().to_os_string();
    part_name.push(".rewind-part");
    let part = target.with_file_name(part_name);

    let result = download_into(client, entry, &part, &target).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&part).await;
        let _ = tokio::fs::remove_file(&target).await;
    }
    result.map(|bytes| RestoredFile { path: target, bytes, sanitized })
}

async fn download_into(client: &KopiaClient, entry: &DirEntry, part: &Path, target: &Path) -> Result<u64> {
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(part)
        .await
        .map_err(|e| Error::io(format!("cannot create {}", part.display()), e))?;
    let mut out = tokio::io::BufWriter::with_capacity(1 << 20, file);
    let mut stream = client.open_object(&entry.obj, None).await?;
    let mut written: u64 = 0;
    while let Some(chunk) = stream.chunk().await? {
        written += chunk.len() as u64;
        out.write_all(&chunk).await.map_err(|e| Error::io(format!("cannot write {}", part.display()), e))?;
    }
    out.flush().await.map_err(|e| Error::io(format!("cannot write {}", part.display()), e))?;
    let file = out.into_inner();
    file.sync_all().await.map_err(|e| Error::io(format!("cannot write {}", part.display()), e))?;
    drop(file);

    if written != entry.size {
        return Err(Error::SizeMismatch { path: target.display().to_string(), expected: entry.size, actual: written });
    }
    // Replaces the empty placeholder created by `reserve`, which only this function owns.
    tokio::fs::rename(part, target)
        .await
        .map_err(|e| Error::io(format!("cannot rename to {}", target.display()), e))?;
    if let Some(mtime) = entry.mtime {
        let modified = std::time::SystemTime::from(mtime);
        let target = target.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let f = std::fs::OpenOptions::new().write(true).open(&target)?;
            f.set_times(std::fs::FileTimes::new().set_modified(modified))
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))?
        .map_err(|e| Error::io("cannot set modification time", e))?;
    }
    Ok(written)
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
    fn builds_restored_names() {
        let time = Local.with_ymd_and_hms(2026, 11, 25, 19, 0, 0).unwrap();
        let naming = Naming::Suffix { label: "restored".into(), time };
        assert_eq!(restored_name("report.pdf", &naming), "report (restored 2026-11-25 19-00).pdf");
        assert_eq!(restored_name("archive.tar.gz", &naming), "archive.tar (restored 2026-11-25 19-00).gz");
        assert_eq!(restored_name(".bashrc", &naming), ".bashrc (restored 2026-11-25 19-00)");
        assert_eq!(restored_name("noext", &naming), "noext (restored 2026-11-25 19-00)");
        assert_eq!(restored_name("x.pdf", &Naming::Original), "x.pdf");
    }

    #[tokio::test]
    async fn reserve_never_reuses_existing_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"keep").unwrap();
        let p = reserve(dir.path(), "a.txt").await.unwrap();
        assert_eq!(p.file_name().unwrap(), "a (2).txt");
        let p = reserve(dir.path(), "a.txt").await.unwrap();
        assert_eq!(p.file_name().unwrap(), "a (3).txt");
        assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), b"keep");
    }
}
