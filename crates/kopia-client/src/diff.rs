//! Content changes between two snapshots of the same source.
//!
//! Kopia object IDs are content hashes: a folder with the same ID in both snapshots has exactly
//! the same content, so it is skipped without being listed (this is how `kopia diff` works).
//! Only folders whose ID differs are listed and compared, so the cost follows the number of
//! changed folders, not the size of the backup.
//!
//! Only changes of **content** are reported: a file with the same object ID but another
//! modification time or permissions is not a change here.

use std::collections::{HashMap, HashSet, VecDeque};

use futures_util::future::try_join_all;

use crate::{DirEntry, DirManifest, EntryType, KopiaClient, Oid, Result};

/// Folder pairs listed at most. Each one costs two requests (cached by object ID).
const MAX_DIRS: usize = 3_000;
/// Changes collected at most; the walk stops once this many were found.
const MAX_CHANGES: usize = 10_000;
/// Folder pairs listed at the same time.
const BATCH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

/// One changed file, or a whole folder that appeared or disappeared (never listed inside).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Path below the snapshot root, components separated by `/`.
    pub path: String,
    pub kind: ChangeKind,
    pub is_dir: bool,
    /// Size in the older snapshot (0 when added). For a folder: the total of its contents.
    pub size_before: u64,
    /// Size in the newer snapshot (0 when removed).
    pub size_after: u64,
    /// Files covered by this row: 1, or the number of files inside a folder.
    pub files: u64,
}

impl Change {
    /// What decides the order of the list: the bigger side of the change.
    fn weight(&self) -> u64 {
        self.size_before.max(self.size_after)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffTotals {
    pub added_files: u64,
    pub removed_files: u64,
    pub modified_files: u64,
    /// Newer total minus older total of the changed items; the "+2,9 MB" of the timeline.
    pub net_bytes: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotDiff {
    /// Biggest change first.
    pub changes: Vec<Change>,
    pub totals: DiffTotals,
    /// The walk stopped at its limits: `changes` and `totals` only cover part of the changes.
    pub truncated: bool,
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() { name.to_string() } else { format!("{prefix}/{name}") }
}

fn whole(prefix: &str, e: &DirEntry, kind: ChangeKind) -> Change {
    let is_dir = e.entry_type == EntryType::Directory;
    let (size, files) = if is_dir { e.summ.as_ref().map_or((0, 0), |s| (s.size, s.files)) } else { (e.size, 1) };
    let (size_before, size_after) = if kind == ChangeKind::Added { (0, size) } else { (size, 0) };
    Change { path: join(prefix, &e.name), kind, is_dir, size_before, size_after, files }
}

/// Compares the listings of two folders. Folders present on both sides with another ID go to
/// `descend` (path, older ID, newer ID); everything else is reported directly.
fn compare(
    prefix: &str,
    old: &DirManifest,
    new: &DirManifest,
    changes: &mut Vec<Change>,
    descend: &mut Vec<(String, Oid, Oid)>,
) {
    let old_by_name: HashMap<&str, &DirEntry> = old.entries.iter().map(|e| (e.name.as_str(), e)).collect();
    let new_names: HashSet<&str> = new.entries.iter().map(|e| e.name.as_str()).collect();

    for n in &new.entries {
        let Some(o) = old_by_name.get(n.name.as_str()) else {
            changes.push(whole(prefix, n, ChangeKind::Added));
            continue;
        };
        let (old_dir, new_dir) = (o.entry_type == EntryType::Directory, n.entry_type == EntryType::Directory);
        match (old_dir, new_dir) {
            (true, true) => {
                if o.obj != n.obj {
                    descend.push((join(prefix, &n.name), o.obj.clone(), n.obj.clone()));
                }
            }
            // A file became a folder or the other way round: one thing gone, another one new.
            (true, false) | (false, true) => {
                changes.push(whole(prefix, o, ChangeKind::Removed));
                changes.push(whole(prefix, n, ChangeKind::Added));
            }
            (false, false) => {
                if o.obj != n.obj || o.entry_type != n.entry_type {
                    changes.push(Change {
                        path: join(prefix, &n.name),
                        kind: ChangeKind::Modified,
                        is_dir: false,
                        size_before: o.size,
                        size_after: n.size,
                        files: 1,
                    });
                }
            }
        }
    }
    for o in &old.entries {
        if !new_names.contains(o.name.as_str()) {
            changes.push(whole(prefix, o, ChangeKind::Removed));
        }
    }
}

fn totals(changes: &[Change]) -> DiffTotals {
    let mut t = DiffTotals::default();
    for c in changes {
        match c.kind {
            ChangeKind::Added => t.added_files += c.files,
            ChangeKind::Removed => t.removed_files += c.files,
            ChangeKind::Modified => t.modified_files += c.files,
        }
        t.net_bytes += c.size_after as i64 - c.size_before as i64;
    }
    t
}

impl KopiaClient {
    /// Content changes from the snapshot with root `old_root` to the one with root `new_root`.
    pub async fn diff_snapshots(&self, old_root: &Oid, new_root: &Oid) -> Result<SnapshotDiff> {
        let mut changes = Vec::new();
        let mut queue: VecDeque<(String, Oid, Oid)> = VecDeque::new();
        if old_root != new_root {
            queue.push_back((String::new(), old_root.clone(), new_root.clone()));
        }
        let mut listed = 0;
        while !queue.is_empty() && listed < MAX_DIRS && changes.len() < MAX_CHANGES {
            let take = BATCH.min(MAX_DIRS - listed).min(queue.len());
            let batch: Vec<_> = queue.drain(..take).collect();
            listed += batch.len();
            let listings = try_join_all(batch.iter().map(|(_, a, b)| async move {
                let (old, new) = futures_util::future::try_join(self.list_directory(a), self.list_directory(b)).await?;
                Ok::<_, crate::Error>((old, new))
            }))
            .await?;
            let mut descend = Vec::new();
            for ((prefix, _, _), (old, new)) in batch.iter().zip(&listings) {
                compare(prefix, old, new, &mut changes, &mut descend);
            }
            queue.extend(descend);
        }
        let truncated = !queue.is_empty();
        changes.sort_by(|a, b| b.weight().cmp(&a.weight()).then_with(|| a.path.cmp(&b.path)));
        Ok(SnapshotDiff { totals: totals(&changes), changes, truncated })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest(entries: serde_json::Value) -> DirManifest {
        serde_json::from_value(json!({ "entries": entries })).unwrap()
    }

    fn file(name: &str, obj: &str, size: u64) -> serde_json::Value {
        json!({"name": name, "type": "f", "obj": obj, "size": size})
    }

    fn dir(name: &str, obj: &str, size: u64, files: u64) -> serde_json::Value {
        json!({"name": name, "type": "d", "obj": obj, "summ": {"size": size, "files": files}})
    }

    fn run(old: serde_json::Value, new: serde_json::Value) -> (Vec<Change>, Vec<(String, Oid, Oid)>) {
        let (mut changes, mut descend) = (Vec::new(), Vec::new());
        compare("a", &manifest(old), &manifest(new), &mut changes, &mut descend);
        (changes, descend)
    }

    #[test]
    fn reports_added_removed_and_modified_files() {
        let (c, d) = run(
            json!([file("same", "k01", 1), file("edit", "k02", 10), file("gone", "k03", 5)]),
            json!([file("same", "k01", 1), file("edit", "k04", 30), file("new", "k05", 7)]),
        );
        assert!(d.is_empty());
        let find = |p: &str| c.iter().find(|x| x.path == p).unwrap();
        assert_eq!(c.len(), 3);
        assert_eq!(find("a/new").kind, ChangeKind::Added);
        assert_eq!((find("a/new").size_before, find("a/new").size_after), (0, 7));
        assert_eq!(find("a/gone").kind, ChangeKind::Removed);
        assert_eq!((find("a/gone").size_before, find("a/gone").size_after), (5, 0));
        assert_eq!(find("a/edit").kind, ChangeKind::Modified);
        assert_eq!((find("a/edit").size_before, find("a/edit").size_after), (10, 30));
    }

    #[test]
    fn same_content_with_other_metadata_is_not_a_change() {
        let old = json!([{"name": "f", "type": "f", "obj": "k01", "size": 1, "mtime": "2026-01-01T00:00:00Z", "mode": "0644"}]);
        let new = json!([{"name": "f", "type": "f", "obj": "k01", "size": 1, "mtime": "2026-02-02T00:00:00Z", "mode": "0600"}]);
        let (c, d) = run(old, new);
        assert!(c.is_empty() && d.is_empty());
    }

    #[test]
    fn folders_with_the_same_id_are_skipped_and_others_are_descended() {
        let (c, d) = run(
            json!([dir("same", "k10", 9, 3), dir("changed", "k11", 9, 3)]),
            json!([dir("same", "k10", 9, 3), dir("changed", "k12", 12, 4)]),
        );
        assert!(c.is_empty());
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, "a/changed");
        assert_eq!((d[0].1.as_str(), d[0].2.as_str()), ("k11", "k12"));
    }

    #[test]
    fn an_added_or_removed_folder_is_one_row() {
        let (c, d) = run(json!([dir("old", "k20", 100, 4)]), json!([dir("new", "k21", 250, 9)]));
        assert!(d.is_empty());
        assert_eq!(c.len(), 2);
        let added = c.iter().find(|x| x.kind == ChangeKind::Added).unwrap();
        assert!(added.is_dir);
        assert_eq!((added.size_after, added.files), (250, 9));
        let removed = c.iter().find(|x| x.kind == ChangeKind::Removed).unwrap();
        assert_eq!((removed.size_before, removed.files), (100, 4));
    }

    #[test]
    fn a_file_that_becomes_a_folder_is_a_removal_plus_an_addition() {
        let (c, d) = run(json!([file("x", "k30", 5)]), json!([dir("x", "k31", 40, 2)]));
        assert!(d.is_empty());
        assert_eq!(c.len(), 2);
        assert!(c.iter().any(|x| x.kind == ChangeKind::Removed && !x.is_dir));
        assert!(c.iter().any(|x| x.kind == ChangeKind::Added && x.is_dir));
    }

    #[test]
    fn totals_add_up_to_the_net_size_change() {
        let changes = vec![
            Change {
                path: "a".into(),
                kind: ChangeKind::Added,
                is_dir: true,
                size_before: 0,
                size_after: 250,
                files: 9,
            },
            Change {
                path: "b".into(),
                kind: ChangeKind::Removed,
                is_dir: false,
                size_before: 5,
                size_after: 0,
                files: 1,
            },
            Change {
                path: "c".into(),
                kind: ChangeKind::Modified,
                is_dir: false,
                size_before: 10,
                size_after: 30,
                files: 1,
            },
        ];
        let t = totals(&changes);
        assert_eq!((t.added_files, t.removed_files, t.modified_files), (9, 1, 1));
        assert_eq!(t.net_bytes, 250 - 5 + 20);
    }
}
