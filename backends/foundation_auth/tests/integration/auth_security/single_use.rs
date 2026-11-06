//! A `ToS` code and an MFA challenge work once, even under concurrent use.

use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use foundation_auth::server::models::{PendingLogin, TosVersion};
use foundation_auth::server::services::{MfaService, TosService};
use foundation_auth::server::storage::HandlerStorage;
use foundation_auth::server::TotpKeyRing;
use foundation_core::valtron::{valtron_test, BlockingFutureExt, MonotonicClock};
use foundation_db::{AsyncListStream, KeyValueStore, MemoryStorage, QueryStore, StorageResult};
use serde::{de::DeserializeOwned, Serialize};

const THREADS: usize = 50;

fn test_clock() -> Arc<MonotonicClock> {
    Arc::new(MonotonicClock::new())
}

/// A cache whose `get` is slow. A read-then-delete take lets every thread read
/// before one deletes; an atomic take does not.
#[derive(Clone)]
struct SlowGet(MemoryStorage);

#[async_trait::async_trait]
impl KeyValueStore for SlowGet {
    async fn get<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        let value = self.0.get(key).await;
        thread::sleep(Duration::from_millis(20));
        value
    }
    async fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        self.0.set(key, value).await
    }
    async fn delete(&self, key: &str) -> StorageResult<()> {
        self.0.delete(key).await
    }
    async fn take<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        self.0.take(key).await
    }
    async fn exists(&self, key: &str) -> StorageResult<bool> {
        self.0.exists(key).await
    }
    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        self.0.list_keys(prefix).await
    }
}

fn storage() -> Arc<HandlerStorage<SlowGet>> {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider =
        crate::integration::turso_provider(dir.path().join("a.db").to_str().unwrap().into());
    std::mem::forget(dir);
    let query_store: Arc<dyn QueryStore> = Arc::new(provider);
    Arc::new(HandlerStorage::new(
        query_store,
        SlowGet(MemoryStorage::new()),
        test_clock(),
    ))
}

fn pending() -> PendingLogin {
    PendingLogin {
        auth_method: "password".into(),
        client_id: None,
        redirect_uri: None,
        state: None,
        nonce: None,
    }
}

/// Run `f` on `THREADS` threads at once; count the `true` results.
fn race<F>(f: F) -> usize
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    let f = Arc::new(f);
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let (f, barrier) = (Arc::clone(&f), Arc::clone(&barrier));
            thread::spawn(move || {
                barrier.wait();
                f()
            })
        })
        .collect();
    handles
        .into_iter()
        .map(|h| h.join().expect("thread"))
        .filter(|ok| *ok)
        .count()
}

#[valtron_test(threads = 8, timeout = 60000)]
fn concurrent_use_of_one_tos_code_lets_exactly_one_through() {
    let storage = storage();
    let tos = Arc::new(TosService::new(
        Arc::clone(&storage),
        Arc::clone(&storage.clock),
    ));
    let code = {
        let tos = Arc::clone(&tos);
        async move {
            tos.add_version(TosVersion {
                version: "v1".into(),
                content: "terms".into(),
                created_at: 1,
                effective_from: 1,
            })
            .await
            .unwrap();
            tos.gate("user-1", pending(), Duration::from_secs(60))
                .await
                .unwrap()
                .expect("a code")
                .code
        }
        .block()
    };

    let winners = race(move || {
        let (tos, code) = (Arc::clone(&tos), code.clone());
        async move { tos.take_code(&code).await.unwrap().is_some() }.block()
    });

    assert_eq!(winners, 1, "one use of the code, not {winners}");
}

#[valtron_test(threads = 8, timeout = 60000)]
fn concurrent_use_of_one_mfa_challenge_lets_exactly_one_through() {
    let storage = storage();
    let mfa = Arc::new(MfaService::new(
        Arc::clone(&storage),
        TotpKeyRing::default(),
        Arc::clone(&storage.clock),
    ));
    let id = {
        let mfa = Arc::clone(&mfa);
        async move {
            mfa.create_challenge("user-1", pending(), Duration::from_secs(60))
                .await
                .unwrap()
        }
        .block()
    };

    let winners = race(move || {
        let (mfa, id) = (Arc::clone(&mfa), id.clone());
        async move { mfa.take_challenge(&id).await.unwrap().is_some() }.block()
    });

    assert_eq!(winners, 1, "one use of the challenge, not {winners}");
}
