//! The in-memory VFS.
//!
//! This VFS lives in `foundation_core`, so it uses the core crate directly. It serves tests,
//! deterministic simulation testing (DST) and small embedded runtimes.

use super::{
    DurableStreamVfs, NotifierRef, StreamName, VfsEntry, VfsError, VfsFuture, VfsMeta, VfsOffset,
};
use std::collections::BTreeMap;
use std::sync::Mutex;

struct MemoryEntry {
    data: Vec<u8>,
    meta: VfsMeta,
}

/// A VFS that keeps every stream in memory.
#[derive(Default)]
pub struct MemoryVfs {
    entries: Mutex<BTreeMap<StreamName, MemoryEntry>>,
}

impl MemoryVfs {
    /// Build an empty VFS.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<StreamName, MemoryEntry>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl DurableStreamVfs for MemoryVfs {
    fn create(&self, name: &StreamName, meta: VfsMeta) -> VfsFuture<'_, bool> {
        let mut entries = self.lock();
        let result = if let Some(existing) = entries.get(name) {
            if existing.meta.content_type != meta.content_type
                || existing.meta.closed != meta.closed
                || existing.meta.expires != meta.expires
            {
                Err(VfsError::Conflict)
            } else {
                Ok(false)
            }
        } else {
            entries.insert(
                name.clone(),
                MemoryEntry {
                    data: Vec::new(),
                    meta,
                },
            );
            Ok(true)
        };
        Box::pin(async move { result })
    }

    fn append(&self, name: &StreamName, bytes: &[u8]) -> VfsFuture<'_, VfsOffset> {
        let mut entries = self.lock();
        let result = match entries.get_mut(name) {
            Some(entry) if entry.meta.closed => Err(VfsError::Conflict),
            Some(entry) => {
                entry.data.extend_from_slice(bytes);
                entry.meta.len = entry.data.len() as u64;
                Ok(VfsOffset(entry.meta.len))
            }
            None => Err(VfsError::NotFound),
        };
        Box::pin(async move { result })
    }

    fn read_at(&self, name: &StreamName, at: &VfsOffset, limit: usize) -> VfsFuture<'_, Vec<u8>> {
        let entries = self.lock();
        let result = match entries.get(name) {
            Some(entry) => {
                let start = usize::try_from(at.0)
                    .unwrap_or(usize::MAX)
                    .min(entry.data.len());
                let end = start.saturating_add(limit).min(entry.data.len());
                Ok(entry.data[start..end].to_vec())
            }
            None => Err(VfsError::NotFound),
        };
        Box::pin(async move { result })
    }

    fn meta(&self, name: &StreamName) -> VfsFuture<'_, VfsMeta> {
        let entries = self.lock();
        let result = entries
            .get(name)
            .map(|entry| entry.meta.clone())
            .ok_or(VfsError::NotFound);
        Box::pin(async move { result })
    }

    fn close(&self, name: &StreamName) -> VfsFuture<'_, ()> {
        let mut entries = self.lock();
        let result = match entries.get_mut(name) {
            Some(entry) => {
                entry.meta.closed = true;
                Ok(())
            }
            None => Err(VfsError::NotFound),
        };
        Box::pin(async move { result })
    }

    fn remove(&self, name: &StreamName) -> VfsFuture<'_, ()> {
        let mut entries = self.lock();
        let result = entries.remove(name).map(|_| ()).ok_or(VfsError::NotFound);
        Box::pin(async move { result })
    }

    fn list(&self, prefix: &StreamName) -> VfsFuture<'_, Vec<VfsEntry>> {
        let entries = self.lock();
        let result = entries
            .iter()
            .filter(|(name, _)| name.as_str().starts_with(prefix.as_str()))
            .map(|(name, entry)| VfsEntry {
                name: name.clone(),
                meta: entry.meta.clone(),
            })
            .collect();
        Box::pin(async move { Ok(result) })
    }

    fn notifier(&self, _name: &StreamName) -> Option<NotifierRef> {
        None
    }
}
