//! A key store over any `foundation_db` key-value backend.
//!
//! Each record is one JSON value under `ewe/keystore/v1/<id>`. `create` uses
//! [`KeyValueStore::insert_if_absent_async`], so two processes cannot both
//! write the same key. A backend without that operation (Workers KV) refuses
//! to create, and the store is then read-only in practice.

use std::sync::Arc;

use foundation_db::{KeyValueStore, StorageError};

use super::{
    ensure_sealed, CreateOutcome, KeyFuture, KeyId, KeyKind, KeyRecord, KeyStore, KeyStoreError,
    KeyStoreInfo,
};

/// The default prefix of a record key.
pub const DEFAULT_PREFIX: &str = "ewe/keystore/v1/";

/// A key store over a `foundation_db` key-value store.
pub struct DbKeyStore<K: KeyValueStore> {
    store: Arc<K>,
    prefix: String,
}

impl<K: KeyValueStore> std::fmt::Debug for DbKeyStore<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbKeyStore")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

impl<K: KeyValueStore> DbKeyStore<K> {
    /// A store over `store` with the default prefix.
    #[must_use]
    pub fn new(store: Arc<K>) -> Self {
        Self {
            store,
            prefix: DEFAULT_PREFIX.to_string(),
        }
    }

    /// A store over `store` with `prefix` in place of the default.
    #[must_use]
    pub fn with_prefix(store: Arc<K>, prefix: impl Into<String>) -> Self {
        Self {
            store,
            prefix: prefix.into(),
        }
    }

    /// The storage key of a record.
    fn storage_key(&self, id: &KeyId) -> String {
        format!("{}{}", self.prefix, id.as_str())
    }

    /// The storage key of the record in `key`.
    fn record_id(&self, key: &str) -> Result<KeyId, KeyStoreError> {
        let id = key
            .strip_prefix(&self.prefix)
            .ok_or_else(|| KeyStoreError::Corrupt(key.to_string()))?;
        KeyId::new(id).map_err(|_| KeyStoreError::Corrupt(key.to_string()))
    }
}

fn storage_error(error: &StorageError) -> KeyStoreError {
    KeyStoreError::Storage(error.to_string())
}

impl<K: KeyValueStore + 'static> KeyStore for DbKeyStore<K> {
    fn get<'a>(&'a self, id: &'a KeyId) -> KeyFuture<'a, Option<KeyRecord>> {
        Box::pin(async move {
            self.store
                .get::<KeyRecord>(&self.storage_key(id))
                .await
                .map_err(|e| storage_error(&e))
        })
    }

    fn create<'a>(&'a self, record: &'a KeyRecord) -> KeyFuture<'a, CreateOutcome> {
        Box::pin(async move {
            ensure_sealed(record, "database")?;
            let key = self.storage_key(&record.id);
            match self
                .store
                .insert_if_absent_async(&key, record.clone())
                .await
            {
                Ok(true) => Ok(CreateOutcome::Created),
                Ok(false) => match self
                    .store
                    .get::<KeyRecord>(&key)
                    .await
                    .map_err(|e| storage_error(&e))?
                {
                    Some(existing) => Ok(CreateOutcome::AlreadyExists(existing)),
                    // The key exists but the record does not parse: a corrupt
                    // record. The store must not overwrite it.
                    None => Err(KeyStoreError::Corrupt(record.id.to_string())),
                },
                Err(StorageError::Unsupported(_)) => Err(KeyStoreError::NotAtomic),
                Err(error) => Err(storage_error(&error)),
            }
        })
    }

    fn replace<'a>(&'a self, record: &'a KeyRecord, expected_revision: u64) -> KeyFuture<'a, ()> {
        Box::pin(async move {
            ensure_sealed(record, "database")?;
            let key = self.storage_key(&record.id);
            let existing = self
                .store
                .get::<KeyRecord>(&key)
                .await
                .map_err(|e| storage_error(&e))?
                .ok_or_else(|| KeyStoreError::NotFound(record.id.to_string()))?;
            if existing.revision != expected_revision {
                return Err(KeyStoreError::Conflict(record.id.to_string()));
            }
            self.store
                .set(&key, record.clone())
                .await
                .map_err(|e| storage_error(&e))
        })
    }

    fn list(&self, kind: KeyKind) -> KeyFuture<'_, Vec<KeyRecord>> {
        Box::pin(async move {
            let keys = self
                .store
                .list_keys(Some(&self.prefix))
                .await
                .map_err(|e| storage_error(&e))?
                .collect_all()
                .await
                .map_err(|e| storage_error(&e))?;
            let mut records = Vec::with_capacity(keys.len());
            for key in keys {
                let id = self.record_id(&key)?;
                if let Some(record) = self
                    .store
                    .get::<KeyRecord>(&key)
                    .await
                    .map_err(|e| storage_error(&e))?
                {
                    if record.id != id || record.kind != kind {
                        continue;
                    }
                    records.push(record);
                }
            }
            Ok(records)
        })
    }

    fn info(&self) -> KeyStoreInfo {
        KeyStoreInfo {
            name: "database",
            atomic_create: true,
            shared: true,
            writable: true,
        }
    }
}
