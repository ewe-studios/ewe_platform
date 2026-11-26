//! The in-memory VFS.
//!
//! This VFS lives in `foundation_core`, so it uses the core crate directly. It serves tests,
//! deterministic simulation testing (DST) and small embedded runtimes.

use super::{
    DurableStreamVfs, Notifier, NotifierRef, StreamName, VfsEntry, VfsError, VfsFuture, VfsMeta,
    VfsOffset,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

struct MemoryEntry {
    data: Vec<u8>,
    meta: VfsMeta,
}

/// A one-shot append notifier.
#[derive(Default)]
struct MemoryNotifier {
    callbacks: Mutex<Vec<Box<dyn Fn() + Send + 'static>>>,
}

impl MemoryNotifier {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Box<dyn Fn() + Send + 'static>>> {
        self.callbacks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Notifier for MemoryNotifier {
    fn register(&self, cb: Box<dyn Fn() + Send + 'static>) {
        self.lock().push(cb);
    }

    fn notify(&self) {
        let callbacks = std::mem::take(&mut *self.lock());
        for callback in callbacks {
            callback();
        }
    }
}

/// A VFS that keeps every stream in memory.
#[derive(Default)]
pub struct MemoryVfs {
    entries: Mutex<BTreeMap<StreamName, MemoryEntry>>,
    notifiers: Mutex<BTreeMap<StreamName, Arc<MemoryNotifier>>>,
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

    fn notifier_for(&self, name: &StreamName) -> Arc<MemoryNotifier> {
        self.notifiers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(name.clone())
            .or_default()
            .clone()
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
            let mut meta = meta;
            meta.version = 0;
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
        let (result, notifier) = {
            let mut entries = self.lock();
            let result = match entries.get_mut(name) {
                Some(entry) if entry.meta.closed => Err(VfsError::Conflict),
                Some(entry) => {
                    entry.data.extend_from_slice(bytes);
                    entry.meta.len = entry.data.len() as u64;
                    entry.meta.version = entry.meta.version.saturating_add(1);
                    Ok(VfsOffset(entry.meta.len))
                }
                None => Err(VfsError::NotFound),
            };
            let notifier = result.is_ok().then(|| self.notifier_for(name));
            (result, notifier)
        };
        if let Some(notifier) = notifier {
            notifier.notify();
        }
        Box::pin(async move { result })
    }

    fn append_with_meta(
        &self,
        name: &StreamName,
        bytes: &[u8],
        meta: VfsMeta,
        expected_version: u64,
    ) -> VfsFuture<'_, VfsOffset> {
        let (result, notifier) = {
            let mut entries = self.lock();
            let result = match entries.get_mut(name) {
                Some(entry) if entry.meta.version != expected_version => Err(VfsError::Conflict),
                Some(entry) if entry.meta.closed => Err(VfsError::Conflict),
                Some(entry) => {
                    entry.data.extend_from_slice(bytes);
                    let mut next = meta;
                    next.len = entry.data.len() as u64;
                    next.version = expected_version.saturating_add(1);
                    entry.meta = next;
                    Ok(VfsOffset(entry.meta.len))
                }
                None => Err(VfsError::NotFound),
            };
            let notifier = result.is_ok().then(|| self.notifier_for(name));
            (result, notifier)
        };
        if let Some(notifier) = notifier {
            notifier.notify();
        }
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
        let (result, notifier) = {
            let mut entries = self.lock();
            let result = match entries.get_mut(name) {
                Some(entry) => {
                    entry.meta.closed = true;
                    entry.meta.version = entry.meta.version.saturating_add(1);
                    Ok(())
                }
                None => Err(VfsError::NotFound),
            };
            let notifier = result.is_ok().then(|| self.notifier_for(name));
            (result, notifier)
        };
        if let Some(notifier) = notifier {
            notifier.notify();
        }
        Box::pin(async move { result })
    }

    fn remove(&self, name: &StreamName) -> VfsFuture<'_, ()> {
        let mut entries = self.lock();
        let result = entries.remove(name).map(|_| ()).ok_or(VfsError::NotFound);
        self.notifiers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name);
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

    fn notifier(&self, name: &StreamName) -> Option<NotifierRef> {
        Some(self.notifier_for(name) as NotifierRef)
    }
}
