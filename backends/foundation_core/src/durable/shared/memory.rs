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
    ref_count: u32,
    soft_deleted: bool,
}

impl MemoryEntry {
    fn base_len(&self) -> u64 {
        self.meta.fork.as_ref().map_or(0, |fork| fork.at)
    }
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

    fn forget_notifier(&self, name: &StreamName) {
        self.notifiers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name);
    }

    /// Remove every soft-deleted entry whose reference count reached zero.
    fn cascade(&self) {
        loop {
            let candidate = {
                let entries = self.lock();
                entries
                    .iter()
                    .find(|(_, entry)| entry.soft_deleted && entry.ref_count == 0)
                    .map(|(name, entry)| {
                        (
                            name.clone(),
                            entry.meta.fork.as_ref().map(|fork| fork.source.clone()),
                        )
                    })
            };

            let Some((name, source)) = candidate else {
                break;
            };

            let mut entries = self.lock();
            entries.remove(&name);
            if let Some(source) = source {
                if let Some(entry) = entries.get_mut(&source) {
                    entry.ref_count = entry.ref_count.saturating_sub(1);
                }
            }
            drop(entries);
            self.forget_notifier(&name);
        }
    }
}

/// Read through the fork chain.
///
/// The chain reads a soft-deleted source on purpose. A living fork depends on
/// that data.
fn read_chain(
    entries: &BTreeMap<StreamName, MemoryEntry>,
    name: &StreamName,
    at: u64,
    limit: usize,
    depth: u32,
) -> Result<Vec<u8>, VfsError> {
    if depth > 64 {
        return Err(VfsError::Conflict);
    }

    let entry = entries.get(name).ok_or(VfsError::NotFound)?;
    if at < entry.meta.trim_from {
        return Err(VfsError::Trimmed);
    }
    let base = entry.base_len();
    let mut out = Vec::new();

    if at < base && limit > 0 {
        let take = usize::try_from(base - at).unwrap_or(usize::MAX).min(limit);
        if let Some(fork) = &entry.meta.fork {
            out.extend_from_slice(&read_chain(entries, &fork.source, at, take, depth + 1)?);
        }
    }

    if out.len() < limit {
        let own_at = if at < base { 0 } else { at - base };
        let start = usize::try_from(own_at)
            .unwrap_or(usize::MAX)
            .min(entry.data.len());
        let take = (limit - out.len()).min(entry.data.len() - start);
        out.extend_from_slice(&entry.data[start..start + take]);
    }

    Ok(out)
}

impl DurableStreamVfs for MemoryVfs {
    fn create(&self, name: &StreamName, meta: VfsMeta) -> VfsFuture<'_, bool> {
        let mut entries = self.lock();
        let result = if let Some(existing) = entries.get(name) {
            if existing.soft_deleted {
                Err(VfsError::Gone)
            } else if existing.meta.content_type != meta.content_type
                || existing.meta.closed != meta.closed
                || existing.meta.expires != meta.expires
                || existing.meta.fork != meta.fork
            {
                Err(VfsError::Conflict)
            } else {
                Ok(false)
            }
        } else {
            let fork_source = meta.fork.as_ref().map(|fork| fork.source.clone());
            if let Some(source) = &fork_source {
                let Some(source_entry) = entries.get(source) else {
                    return Box::pin(async { Err(VfsError::NotFound) });
                };
                if source_entry.soft_deleted {
                    return Box::pin(async { Err(VfsError::Gone) });
                }
                if meta.len > source_entry.meta.len {
                    return Box::pin(async { Err(VfsError::Conflict) });
                }
            }

            let mut meta = meta;
            meta.version = 0;
            if let Some(source) = fork_source {
                if let Some(source_entry) = entries.get_mut(&source) {
                    source_entry.ref_count = source_entry.ref_count.saturating_add(1);
                }
            }
            entries.insert(
                name.clone(),
                MemoryEntry {
                    data: Vec::new(),
                    meta,
                    ref_count: 0,
                    soft_deleted: false,
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
                Some(entry) if entry.soft_deleted => Err(VfsError::Gone),
                Some(entry) if entry.meta.closed => Err(VfsError::Conflict),
                Some(entry) => {
                    entry.data.extend_from_slice(bytes);
                    entry.meta.len = entry.base_len() + entry.data.len() as u64;
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
                Some(entry) if entry.soft_deleted => Err(VfsError::Gone),
                Some(entry) if entry.meta.version != expected_version => Err(VfsError::Conflict),
                Some(entry) if entry.meta.closed => Err(VfsError::Conflict),
                Some(entry) => {
                    entry.data.extend_from_slice(bytes);
                    let mut next = meta;
                    next.len = entry.base_len() + entry.data.len() as u64;
                    next.fork = entry.meta.fork.clone();
                    next.trim_from = entry.meta.trim_from;
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

    /// A direct read of a soft-deleted entry returns `Gone`. The internal chain
    /// still reads a soft-deleted source, because a living fork depends on it.
    fn read_at(&self, name: &StreamName, at: &VfsOffset, limit: usize) -> VfsFuture<'_, Vec<u8>> {
        let entries = self.lock();
        let result = match entries.get(name) {
            Some(entry) if entry.soft_deleted => Err(VfsError::Gone),
            _ => read_chain(&entries, name, at.0, limit, 0),
        };
        Box::pin(async move { result })
    }

    fn meta(&self, name: &StreamName) -> VfsFuture<'_, VfsMeta> {
        let entries = self.lock();
        let result = match entries.get(name) {
            Some(entry) if entry.soft_deleted => Err(VfsError::Gone),
            Some(entry) => Ok(entry.meta.clone()),
            None => Err(VfsError::NotFound),
        };
        Box::pin(async move { result })
    }

    fn trim(&self, name: &StreamName, at: VfsOffset) -> VfsFuture<'_, ()> {
        let mut entries = self.lock();
        let result = match entries.get_mut(name) {
            Some(entry) if entry.soft_deleted => Err(VfsError::Gone),
            Some(entry) => {
                entry.meta.trim_from = at.0.min(entry.meta.len).max(entry.meta.trim_from);
                entry.meta.version = entry.meta.version.saturating_add(1);
                Ok(())
            }
            None => Err(VfsError::NotFound),
        };
        Box::pin(async move { result })
    }

    fn touch(&self, name: &StreamName, now_ms: i64) -> VfsFuture<'_, ()> {
        let mut entries = self.lock();
        let result = match entries.get_mut(name) {
            Some(entry) if entry.soft_deleted => Err(VfsError::Gone),
            Some(entry) => {
                entry.meta.last_access_ms = now_ms;
                Ok(())
            }
            None => Err(VfsError::NotFound),
        };
        Box::pin(async move { result })
    }

    fn close(&self, name: &StreamName) -> VfsFuture<'_, ()> {
        let (result, notifier) = {
            let mut entries = self.lock();
            let result = match entries.get_mut(name) {
                Some(entry) if entry.soft_deleted => Err(VfsError::Gone),
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

    fn remove(&self, name: &StreamName) -> VfsFuture<'_, bool> {
        let (result, notifier) = {
            let mut entries = self.lock();
            let result = match entries.get(name) {
                None => Err(VfsError::NotFound),
                Some(entry) if entry.soft_deleted => Err(VfsError::Gone),
                Some(entry) => {
                    let source = entry.meta.fork.as_ref().map(|fork| fork.source.clone());
                    let soft = entry.ref_count > 0;
                    if soft {
                        let entry = entries.get_mut(name).expect("entry");
                        entry.soft_deleted = true;
                        entry.meta.version = entry.meta.version.saturating_add(1);
                    } else {
                        entries.remove(name);
                        // A soft-deleted entry keeps its source reference until it is
                        // collected, because a deeper fork still reads through it.
                        if let Some(source) = source {
                            if let Some(entry) = entries.get_mut(&source) {
                                entry.ref_count = entry.ref_count.saturating_sub(1);
                            }
                        }
                    }
                    Ok(soft)
                }
            };
            let notifier = result
                .as_ref()
                .is_ok_and(|soft| *soft)
                .then(|| self.notifier_for(name));
            (result, notifier)
        };

        if result.is_ok() {
            if !result.as_ref().is_ok_and(|soft| *soft) {
                self.forget_notifier(name);
            }
            self.cascade();
        }
        if let Some(notifier) = notifier {
            notifier.notify();
        }
        Box::pin(async move { result })
    }

    fn list(&self, prefix: &StreamName) -> VfsFuture<'_, Vec<VfsEntry>> {
        let entries = self.lock();
        let result = entries
            .iter()
            .filter(|(name, entry)| {
                !entry.soft_deleted && name.as_str().starts_with(prefix.as_str())
            })
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
