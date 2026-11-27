//! An in-memory key store for tests and deterministic simulation.

use std::collections::HashMap;
use std::sync::Mutex;

use super::{
    ensure_sealed, CreateOutcome, KeyFuture, KeyId, KeyKind, KeyRecord, KeyStore, KeyStoreError,
    KeyStoreInfo,
};

/// A key store in memory. `create` is atomic; the store is not shared.
///
/// It is the store of a deterministic simulation test. A real host uses
/// [`FsKeyStore`](super::FsKeyStore) or [`DbKeyStore`](super::DbKeyStore).
#[derive(Default)]
pub struct MemoryKeyStore {
    records: Mutex<HashMap<KeyId, KeyRecord>>,
}

impl std::fmt::Debug for MemoryKeyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MemoryKeyStore")
    }
}

impl MemoryKeyStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<KeyId, KeyRecord>>, KeyStoreError> {
        self.records
            .lock()
            .map_err(|e| KeyStoreError::Storage(format!("Mutex poisoned: {e}")))
    }
}

impl KeyStore for MemoryKeyStore {
    fn get<'a>(&'a self, id: &'a KeyId) -> KeyFuture<'a, Option<KeyRecord>> {
        Box::pin(async move { Ok(self.lock()?.get(id).cloned()) })
    }

    fn create<'a>(&'a self, record: &'a KeyRecord) -> KeyFuture<'a, CreateOutcome> {
        Box::pin(async move {
            ensure_sealed(record, "memory")?;
            let mut records = self.lock()?;
            // One lock covers the lookup and the insert.
            if let Some(existing) = records.get(&record.id) {
                Ok(CreateOutcome::AlreadyExists(existing.clone()))
            } else {
                records.insert(record.id.clone(), record.clone());
                Ok(CreateOutcome::Created)
            }
        })
    }

    fn replace<'a>(&'a self, record: &'a KeyRecord, expected_revision: u64) -> KeyFuture<'a, ()> {
        Box::pin(async move {
            ensure_sealed(record, "memory")?;
            let mut records = self.lock()?;
            match records.get(&record.id) {
                None => Err(KeyStoreError::NotFound(record.id.to_string())),
                Some(existing) if existing.revision != expected_revision => {
                    Err(KeyStoreError::Conflict(record.id.to_string()))
                }
                Some(_) => {
                    records.insert(record.id.clone(), record.clone());
                    Ok(())
                }
            }
        })
    }

    fn list(&self, kind: KeyKind) -> KeyFuture<'_, Vec<KeyRecord>> {
        Box::pin(async move {
            Ok(self
                .lock()?
                .values()
                .filter(|record| record.kind == kind)
                .cloned()
                .collect())
        })
    }

    fn info(&self) -> KeyStoreInfo {
        KeyStoreInfo {
            name: "memory",
            atomic_create: true,
            shared: false,
            writable: true,
        }
    }
}
