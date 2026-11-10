//! The `IdP` refuses a cache that cannot make single-use data single use.

use std::sync::Arc;

use foundation_auth::server::storage::HandlerStorage;
use foundation_auth::server::{IdpConfig, IdpServer};
use foundation_core::valtron::{valtron_test, Ctx, MonotonicClock};
use foundation_db::{AsyncListStream, KeyValueStore, MemoryStorage, QueryStore, StorageResult};
use serde::{de::DeserializeOwned, Serialize};

fn test_clock() -> Ctx {
    Ctx::fixed(Arc::new(MonotonicClock::new()))
}

/// A cache like Workers KV: every operation works, but `take` keeps the
/// default read-then-delete (`take_is_atomic` is false).
#[derive(Clone)]
struct ReadThenDelete(MemoryStorage);

#[async_trait::async_trait]
impl KeyValueStore for ReadThenDelete {
    async fn get<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        self.0.get(key).await
    }
    async fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        self.0.set(key, value).await
    }
    async fn delete(&self, key: &str) -> StorageResult<()> {
        self.0.delete(key).await
    }
    async fn exists(&self, key: &str) -> StorageResult<bool> {
        self.0.exists(key).await
    }
    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        self.0.list_keys(prefix).await
    }
}

fn storage<KV: KeyValueStore>(cache: KV, ctx: Ctx) -> Arc<HandlerStorage<KV>> {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider =
        crate::integration::turso_provider(dir.path().join("g.db").to_str().unwrap().into());
    std::mem::forget(dir);
    let query_store: Arc<dyn QueryStore> = Arc::new(provider);
    Arc::new(HandlerStorage::new(query_store, cache, ctx))
}

fn config() -> IdpConfig {
    IdpConfig::new("https://auth.example.com".into())
}

#[valtron_test(threads = 8, timeout = 60000)]
fn idp_refuses_a_cache_without_atomic_take() {
    let ctx = test_clock();
    let refused = IdpServer::try_new(
        config(),
        storage(ReadThenDelete(MemoryStorage::new()), ctx.clone()),
        ctx,
    );
    let error = refused
        .err()
        .expect("the cache must be refused")
        .to_string();
    assert!(error.contains("atomic"), "{error}");
    assert!(error.contains("allow_non_atomic_cache"), "{error}");
}

#[valtron_test(threads = 8, timeout = 60000)]
fn the_risk_can_be_accepted_with_a_flag() {
    let ctx = test_clock();
    let accepted = IdpServer::try_new(
        config().with_allow_non_atomic_cache(true),
        storage(ReadThenDelete(MemoryStorage::new()), ctx.clone()),
        ctx,
    );
    assert!(accepted.is_ok());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn atomic_caches_are_accepted() {
    let ctx = test_clock();
    assert!(IdpServer::try_new(config(), storage(MemoryStorage::new(), ctx.clone()), ctx).is_ok());
}
