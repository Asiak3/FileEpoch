//! Cache of directory listings keyed by object ID.
//!
//! Kopia object IDs are content hashes, so a listing for a given ID never changes and can be
//! cached without invalidation. Identical folders in different snapshots share the same ID, which
//! makes moving along the timeline cheap. The cache is bounded by the total number of entries.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{DirManifest, Oid};

/// Default bound: about 250k entries (roughly 50–100 MB).
pub(crate) const DEFAULT_MAX_ENTRIES: usize = 250_000;

pub(crate) struct DirCache {
    inner: Mutex<Inner>,
    max_entries: usize,
}

struct Inner {
    map: HashMap<Oid, (Arc<DirManifest>, u64)>,
    total_entries: usize,
    clock: u64,
}

impl DirCache {
    pub(crate) fn new(max_entries: usize) -> Self {
        DirCache { inner: Mutex::new(Inner { map: HashMap::new(), total_entries: 0, clock: 0 }), max_entries }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn get(&self, oid: &Oid) -> Option<Arc<DirManifest>> {
        let mut g = self.lock();
        g.clock += 1;
        let now = g.clock;
        g.map.get_mut(oid).map(|(m, used)| {
            *used = now;
            m.clone()
        })
    }

    pub(crate) fn insert(&self, oid: Oid, manifest: Arc<DirManifest>) {
        let size = manifest.entries.len().max(1);
        let mut g = self.lock();
        g.clock += 1;
        let now = g.clock;
        if let Some((old, _)) = g.map.insert(oid, (manifest, now)) {
            g.total_entries -= old.entries.len().max(1);
        }
        g.total_entries += size;
        // Evict least recently used listings, but always keep the one just inserted.
        while g.total_entries > self.max_entries && g.map.len() > 1 {
            let Some(victim) = g
                .map
                .iter()
                .filter(|(_, (_, used))| *used != now)
                .min_by_key(|(_, (_, used))| *used)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some((m, _)) = g.map.remove(&victim) {
                g.total_entries -= m.entries.len().max(1);
            }
        }
    }

    pub(crate) fn clear(&self) {
        let mut g = self.lock();
        g.map.clear();
        g.total_entries = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(n: usize) -> Arc<DirManifest> {
        let entries = (0..n)
            .map(|i| {
                serde_json::from_value(serde_json::json!({"name": format!("f{i}"), "type": "f", "obj": "0a0b"}))
                    .unwrap()
            })
            .collect();
        Arc::new(DirManifest { stream: String::new(), entries, summary: None })
    }

    fn oid(s: &str) -> Oid {
        s.parse().unwrap()
    }

    #[test]
    fn evicts_least_recently_used_beyond_bound() {
        let c = DirCache::new(10);
        c.insert(oid("k01"), manifest(4));
        c.insert(oid("k02"), manifest(4));
        assert!(c.get(&oid("k01")).is_some()); // k01 now more recent than k02
        c.insert(oid("k03"), manifest(4)); // 12 > 10: evict k02
        assert!(c.get(&oid("k02")).is_none());
        assert!(c.get(&oid("k01")).is_some());
        assert!(c.get(&oid("k03")).is_some());
    }

    #[test]
    fn keeps_a_single_oversized_listing() {
        let c = DirCache::new(10);
        c.insert(oid("k01"), manifest(3));
        c.insert(oid("k02"), manifest(50));
        assert!(c.get(&oid("k02")).is_some());
        assert!(c.get(&oid("k01")).is_none());
    }
}
