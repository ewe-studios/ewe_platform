//! The race: two processes that start at the same time agree on one key.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use foundation_auth::keystore::{
    CreateOutcome, KekSealer, KeyFuture, KeyId, KeyKind, KeyMaterial, KeyProtection, KeyRecord,
    KeySealer, KeyStore, KeyStoreError, KeyStoreInfo, MemoryKeyStore,
};
use foundation_core::valtron::valtron_test;

use super::common::test_ctx;

/// A store whose first `get` returns `None` even when it holds the record.
///
/// It makes the "another writer won" branch of `KeyMaterial::load_or_create`
/// reachable in one process.
struct StaleGetStore {
    inner: MemoryKeyStore,
    stale: AtomicBool,
}

impl StaleGetStore {
    fn new() -> Self {
        Self {
            inner: MemoryKeyStore::new(),
            stale: AtomicBool::new(false),
        }
    }

    fn answer_stale(&self) {
        self.stale.store(true, Ordering::SeqCst);
    }
}

impl KeyStore for StaleGetStore {
    fn get<'a>(&'a self, id: &'a KeyId) -> KeyFuture<'a, Option<KeyRecord>> {
        Box::pin(async move {
            if self.stale.swap(false, Ordering::SeqCst) {
                return Ok(None);
            }
            self.inner.get(id).await
        })
    }

    fn create<'a>(&'a self, record: &'a KeyRecord) -> KeyFuture<'a, CreateOutcome> {
        self.inner.create(record)
    }

    fn replace<'a>(&'a self, record: &'a KeyRecord, expected_revision: u64) -> KeyFuture<'a, ()> {
        self.inner.replace(record, expected_revision)
    }

    fn list(&self, kind: KeyKind) -> KeyFuture<'_, Vec<KeyRecord>> {
        self.inner.list(kind)
    }

    fn info(&self) -> KeyStoreInfo {
        self.inner.info()
    }
}

fn sealer(env: &foundation_core::env::Env) -> Arc<dyn KeySealer> {
    Arc::new(KekSealer::new("race-kek", [3; 32], env.clone()))
}

#[valtron_test]
async fn a_second_start_uses_the_key_that_is_already_there() {
    let (ctx, env) = test_ctx();
    let store = Arc::new(MemoryKeyStore::new());
    let id = KeyId::new(KeyId::SECRET_SIGNING).expect("valid id");

    let first = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(sealer(&env)),
        ctx.clone(),
    );
    let second = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(sealer(&env)),
        ctx.clone(),
    );

    let a = first.signing_key(&id).await.expect("first key");
    let b = second.signing_key(&id).await.expect("second key");
    assert_eq!(
        a.verifying_key().kid(),
        b.verifying_key().kid(),
        "both processes use the same key"
    );
    assert_eq!(
        store
            .list(KeyKind::SecretSigning)
            .await
            .expect("list")
            .len(),
        1
    );
}

#[valtron_test]
async fn a_lost_create_uses_the_winner() {
    let (ctx, env) = test_ctx();
    let store = Arc::new(StaleGetStore::new());
    let id = KeyId::new(KeyId::SECRET_SIGNING).expect("valid id");

    let winner = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(sealer(&env)),
        ctx.clone(),
    );
    let loser = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(sealer(&env)),
        ctx.clone(),
    );

    let first = winner.signing_key(&id).await.expect("winner key");
    // The loser reads no record, makes a key, and loses the create.
    store.answer_stale();
    let second = loser.signing_key(&id).await.expect("loser key");
    assert_eq!(
        first.verifying_key().kid(),
        second.verifying_key().kid(),
        "the losing process drops its key and uses the winner"
    );
    assert_eq!(
        store
            .inner
            .list(KeyKind::SecretSigning)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[valtron_test]
async fn a_verify_only_host_never_makes_a_key() {
    let (ctx, _env) = test_ctx();
    let store = Arc::new(MemoryKeyStore::new());
    let material = KeyMaterial::verify_only(Arc::clone(&store) as Arc<dyn KeyStore>, ctx);
    let id = KeyId::new(KeyId::SECRET_SIGNING).expect("valid id");
    assert!(matches!(
        material.signing_key(&id).await,
        Err(KeyStoreError::ReadOnly)
    ));
}
