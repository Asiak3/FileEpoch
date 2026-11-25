//! Response types. Field names follow Kopia's JSON tags (see `docs/API.md` §3–§6).
//!
//! Kopia omits zero values (`omitempty`), so almost everything is optional or defaulted.
//! Unknown fields are ignored. Timestamps are UTC; convert to local time only for display.

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::Oid;

/// `GET /api/v1/repo/status`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub connected: bool,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub format_version: Option<u32>,
    #[serde(default)]
    pub storage: Option<String>,
    /// Non-empty while the server is opening the repository.
    #[serde(default, rename = "initTaskID")]
    pub init_task_id: Option<String>,
}

/// A snapshot source: `user@host:path`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
pub struct SourceInfo {
    pub host: String,
    #[serde(rename = "userName")]
    pub user_name: String,
    pub path: String,
}

impl std::fmt::Display for SourceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}:{}", self.user_name, self.host, self.path)
    }
}

/// `GET /api/v1/sources`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesResponse {
    #[serde(default)]
    pub local_username: String,
    #[serde(default)]
    pub local_host: String,
    #[serde(default)]
    pub multi_user: bool,
    #[serde(default)]
    pub sources: Vec<SourceStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub source: SourceInfo,
    /// `REMOTE` on a read-only server.
    #[serde(default)]
    pub status: String,
    /// Only refreshed when the server starts or refreshes: do not use it to find the newest
    /// snapshot, list the snapshots instead.
    #[serde(default)]
    pub last_snapshot: Option<SnapshotManifest>,
}

/// The raw snapshot manifest embedded in `sources[].lastSnapshot`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotManifest {
    pub id: String,
    pub start_time: DateTime<Utc>,
    #[serde(default)]
    pub end_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub root_entry: Option<DirEntry>,
}

/// `GET /api/v1/snapshots`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotsResponse {
    /// Oldest first.
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
    #[serde(default)]
    pub unfiltered_count: usize,
    #[serde(default)]
    pub unique_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub start_time: DateTime<Utc>,
    #[serde(default)]
    pub end_time: Option<DateTime<Utc>>,
    /// Set for incomplete snapshots (checkpoints).
    #[serde(default)]
    pub incomplete: Option<String>,
    #[serde(default)]
    pub summary: Option<DirectorySummary>,
    #[serde(rename = "rootID")]
    pub root_id: Oid,
    #[serde(default, deserialize_with = "null_as_default")]
    pub retention: Vec<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub pins: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorySummary {
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub files: u64,
    #[serde(default)]
    pub symlinks: u64,
    #[serde(default)]
    pub dirs: u64,
    #[serde(default)]
    pub max_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub incomplete: Option<String>,
    #[serde(default)]
    pub num_failed: u64,
}

/// `GET /api/v1/objects/{oid}` for a directory.
#[derive(Debug, Clone, Deserialize)]
pub struct DirManifest {
    #[serde(default)]
    pub stream: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub entries: Vec<DirEntry>,
    #[serde(default)]
    pub summary: Option<DirectorySummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum EntryType {
    #[serde(rename = "f")]
    File,
    #[serde(rename = "d")]
    Directory,
    #[serde(rename = "s")]
    Symlink,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DirEntry {
    pub name: String,
    #[serde(rename = "type", default = "unknown_type")]
    pub entry_type: EntryType,
    /// Octal permission string such as `"0644"`.
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub mtime: Option<DateTime<Utc>>,
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub gid: u32,
    pub obj: Oid,
    /// Present for directories.
    #[serde(default)]
    pub summ: Option<DirectorySummary>,
}

fn unknown_type() -> EntryType {
    EntryType::Unknown
}

fn null_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerates_omitted_fields_and_fraction_lengths() {
        let j = r#"{
            "stream": "kopia:directory",
            "entries": [
                {"name": "a", "type": "f", "obj": "0123", "mtime": "2025-11-15T16:50:00Z"},
                {"name": "b", "type": "d", "obj": "k0123", "mtime": "2026-05-16T04:41:17.514896Z",
                 "mode": "0755", "summ": {"size": 5, "files": 1, "maxTime": "2026-05-16T04:41:17.1Z"}},
                {"name": "c", "type": "s", "obj": "0a0b", "mtime": "2026-11-24T22:57:19.636406861Z"},
                {"name": "d", "type": "x", "obj": "0a0b"},
                {"name": "e", "obj": "0a0b"}
            ]
        }"#;
        let m: DirManifest = serde_json::from_str(j).unwrap();
        assert_eq!(m.entries.len(), 5);
        assert_eq!(m.entries[0].entry_type, EntryType::File);
        assert_eq!(m.entries[0].size, 0);
        assert!(m.entries[0].mode.is_none());
        assert_eq!(m.entries[1].summ.as_ref().unwrap().files, 1);
        assert_eq!(m.entries[2].entry_type, EntryType::Symlink);
        assert_eq!(m.entries[2].mtime.unwrap().timestamp_subsec_nanos(), 636_406_861);
        assert_eq!(m.entries[3].entry_type, EntryType::Unknown);
        assert_eq!(m.entries[4].entry_type, EntryType::Unknown);
        assert!(m.summary.is_none());
    }

    #[test]
    fn null_lists_become_empty() {
        let j = r#"{"snapshots": [{"id": "x", "startTime": "2026-01-01T00:00:00Z",
                     "rootID": "k00", "retention": null, "pins": null, "summary": null}]}"#;
        let r: SnapshotsResponse = serde_json::from_str(j).unwrap();
        assert!(r.snapshots[0].retention.is_empty());
        assert!(r.snapshots[0].pins.is_empty());
        let m: DirManifest = serde_json::from_str(r#"{"entries": null}"#).unwrap();
        assert!(m.entries.is_empty());
    }

    #[test]
    fn rejects_invalid_oid_in_response() {
        let j = r#"{"name": "a", "type": "f", "obj": "../../repo/disconnect"}"#;
        assert!(serde_json::from_str::<DirEntry>(j).is_err());
    }
}
