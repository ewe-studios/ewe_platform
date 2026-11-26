//! A read-only key store over values from configuration.
//!
//! A verify-only host such as a Worker reads its public keys from an
//! environment variable, a Worker variable or a Worker secret. It holds no
//! private key, so every write fails with [`KeyStoreError::ReadOnly`].

use super::{
    CreateOutcome, KeyFuture, KeyId, KeyKind, KeyRecord, KeyStore, KeyStoreError, KeyStoreInfo,
};

/// A key store over a fixed list of records. It is read-only.
pub struct EnvKeyStore {
    records: Vec<KeyRecord>,
}

impl std::fmt::Debug for EnvKeyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvKeyStore")
            .field("records", &self.records.len())
            .finish_non_exhaustive()
    }
}

impl EnvKeyStore {
    /// A store over `records`.
    #[must_use]
    pub fn new(records: Vec<KeyRecord>) -> Self {
        Self { records }
    }
}

impl KeyStore for EnvKeyStore {
    fn get<'a>(&'a self, id: &'a KeyId) -> KeyFuture<'a, Option<KeyRecord>> {
        Box::pin(async move { Ok(self.records.iter().find(|record| &record.id == id).cloned()) })
    }

    fn create<'a>(&'a self, _record: &'a KeyRecord) -> KeyFuture<'a, CreateOutcome> {
        Box::pin(async { Err(KeyStoreError::ReadOnly) })
    }

    fn replace<'a>(&'a self, _record: &'a KeyRecord, _expected_revision: u64) -> KeyFuture<'a, ()> {
        Box::pin(async { Err(KeyStoreError::ReadOnly) })
    }

    fn list(&self, kind: KeyKind) -> KeyFuture<'_, Vec<KeyRecord>> {
        Box::pin(async move {
            Ok(self
                .records
                .iter()
                .filter(|record| record.kind == kind)
                .cloned()
                .collect())
        })
    }

    fn info(&self) -> KeyStoreInfo {
        KeyStoreInfo {
            name: "environment",
            atomic_create: false,
            shared: false,
            writable: false,
        }
    }
}
