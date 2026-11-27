//! The `KeyStore` contract: create, get, replace, list and refusals.

use std::sync::Arc;

use foundation_auth::keystore::{
    DbKeyStore, FsKeyStore, KeyId, KeyKind, KeyStore, KeyStoreError, MemoryKeyStore,
};
use foundation_core::valtron::valtron_test;
use foundation_db::{KeyValueStore, MemoryJsonStore, MemoryStorage, TursoStorage};
use tempfile::TempDir;

use super::common::{dummy_blob, plaintext_record, sealed_record};

/// The contract that every writable store must follow.
async fn check_contract<S: KeyStore>(store: S) {
    let id = KeyId::new("ca").expect("valid id");
    let first = sealed_record("ca", KeyKind::CertificateAuthority, dummy_blob());

    assert!(matches!(
        store.create(&first).await.expect("create"),
        foundation_auth::keystore::CreateOutcome::Created
    ));
    let found = store.get(&id).await.expect("get").expect("the record");
    assert_eq!(found.revision, 1);

    let mut second = first.clone();
    second.revision = 2;
    match store.create(&second).await.expect("second create") {
        foundation_auth::keystore::CreateOutcome::AlreadyExists(winner) => {
            assert_eq!(winner.revision, 1);
        }
        foundation_auth::keystore::CreateOutcome::Created => panic!("create must not overwrite"),
    }

    store.replace(&second, 1).await.expect("replace");
    assert_eq!(
        store.get(&id).await.expect("get").expect("record").revision,
        2
    );
    assert!(matches!(
        store.replace(&second, 1).await,
        Err(KeyStoreError::Conflict(_))
    ));

    let absent = KeyId::new("absent").expect("valid id");
    assert!(store.get(&absent).await.expect("get").is_none());
    let absent_record = sealed_record("absent", KeyKind::CertificateAuthority, dummy_blob());
    assert!(matches!(
        store.replace(&absent_record, 1).await,
        Err(KeyStoreError::NotFound(_))
    ));

    let listed = store
        .list(KeyKind::CertificateAuthority)
        .await
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);

    // A clear-text private key is refused by every writable store.
    let clear = plaintext_record("clear", KeyKind::SecretSigning);
    assert!(matches!(
        store.create(&clear).await,
        Err(KeyStoreError::PlaintextRefused(_))
    ));
}

#[valtron_test]
async fn memory_store_follows_the_contract() {
    check_contract(MemoryKeyStore::new()).await;
}

#[valtron_test]
async fn fs_store_follows_the_contract() {
    let dir = TempDir::new().expect("temp dir");
    check_contract(FsKeyStore::new(dir.path())).await;
}

#[valtron_test]
async fn db_store_over_memory_follows_the_contract() {
    check_contract(DbKeyStore::new(Arc::new(MemoryStorage::new()))).await;
}

#[valtron_test]
async fn db_store_over_memory_json_follows_the_contract() {
    check_contract(DbKeyStore::new(Arc::new(MemoryJsonStore::new()))).await;
}

#[valtron_test]
async fn db_store_over_turso_follows_the_contract() {
    let store = TursoStorage::in_memory(foundation_context::Ctx::host())
        .await
        .expect("in-memory Turso store");
    check_contract(DbKeyStore::new(Arc::new(store))).await;
}

/// The read-only store refuses a write and returns its records.
#[valtron_test]
async fn env_store_is_read_only() {
    let env_store = foundation_auth::keystore::EnvKeyStore::new(vec![sealed_record(
        "ca",
        KeyKind::CertificateAuthority,
        dummy_blob(),
    )]);
    assert!(matches!(
        env_store
            .create(&sealed_record(
                "secret-signing",
                KeyKind::SecretSigning,
                dummy_blob()
            ))
            .await,
        Err(KeyStoreError::ReadOnly)
    ));
    assert_eq!(
        env_store
            .list(KeyKind::CertificateAuthority)
            .await
            .expect("list")
            .len(),
        1
    );
}

/// A backend without an atomic create reports `NotAtomic`; Workers KV is one.
#[valtron_test]
async fn a_backend_without_atomic_create_reports_not_atomic() {
    struct NoAtomic;
    #[async_trait::async_trait]
    impl KeyValueStore for NoAtomic {
        async fn get<V: serde::de::DeserializeOwned + Send + 'static>(
            &self,
            _key: &str,
        ) -> foundation_db::StorageResult<Option<V>> {
            Ok(None)
        }
        async fn set<V: serde::Serialize + Send + 'static>(
            &self,
            _key: &str,
            _value: V,
        ) -> foundation_db::StorageResult<()> {
            Ok(())
        }
        async fn delete(&self, _key: &str) -> foundation_db::StorageResult<()> {
            Ok(())
        }
        async fn exists(&self, _key: &str) -> foundation_db::StorageResult<bool> {
            Ok(false)
        }
        async fn list_keys(
            &self,
            _prefix: Option<&str>,
        ) -> foundation_db::StorageResult<foundation_db::AsyncListStream> {
            Ok(foundation_db::AsyncListStream::new(
                futures_util::stream::iter(Vec::<foundation_db::StorageResult<String>>::new()),
            ))
        }
    }

    let store = DbKeyStore::new(Arc::new(NoAtomic));
    assert!(matches!(
        store
            .create(&sealed_record(
                "ca",
                KeyKind::CertificateAuthority,
                dummy_blob()
            ))
            .await,
        Err(KeyStoreError::NotAtomic)
    ));
}
