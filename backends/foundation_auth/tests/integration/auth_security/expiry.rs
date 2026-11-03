//! Expired codes and challenges are not returned, and cleanup deletes them.

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::server::models::{PendingLogin, TosVersion};
use foundation_auth::server::services::{MfaService, TosService};
use foundation_auth::server::storage::HandlerStorage;
use foundation_auth::server::TotpKeyRing;
use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_db::{KeyValueStore, MemoryStorage, QueryStore};
use futures_lite::StreamExt;

fn storage() -> Arc<HandlerStorage<MemoryStorage>> {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider =
        crate::integration::turso_provider(dir.path().join("e.db").to_str().unwrap().into());
    std::mem::forget(dir);
    let query_store: Arc<dyn QueryStore> = Arc::new(provider);
    Arc::new(HandlerStorage::new(query_store, MemoryStorage::new()))
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

async fn keys(storage: &HandlerStorage<MemoryStorage>) -> Vec<String> {
    storage
        .cache
        .list_keys(None)
        .await
        .unwrap()
        .map(Result::unwrap)
        .collect()
        .await
}

#[valtron_test(threads = 8, timeout = 60000)]
fn expired_entries_are_not_returned_and_are_cleaned() {
    let storage = storage();
    let tos = TosService::new(Arc::clone(&storage));
    let mfa = MfaService::new(Arc::clone(&storage), TotpKeyRing::default());
    let storage_c = Arc::clone(&storage);

    async move {
        tos.add_version(TosVersion {
            version: "v1".into(),
            content: "terms".into(),
            created_at: 1,
            effective_from: 1,
        })
        .await
        .unwrap();

        // Two entries that expire at once, two that stay valid.
        let old_code = tos
            .gate("u-old", pending(), Duration::from_millis(1))
            .await
            .unwrap()
            .unwrap()
            .code;
        let old_challenge = mfa
            .create_challenge("u-old", pending(), Duration::from_millis(1))
            .await
            .unwrap();
        let live_code = tos
            .gate("u-live", pending(), Duration::from_secs(600))
            .await
            .unwrap()
            .unwrap()
            .code;
        let live_challenge = mfa
            .create_challenge("u-live", pending(), Duration::from_secs(600))
            .await
            .unwrap();
        std::thread::sleep(Duration::from_millis(50));

        // A read after expiry returns nothing ...
        assert!(tos.take_code(&old_code).await.unwrap().is_none());
        assert!(mfa.take_challenge(&old_challenge).await.unwrap().is_none());

        // ... but entries nobody presents stay in the store until cleanup.
        let stale_code = tos
            .gate("u-stale", pending(), Duration::from_millis(1))
            .await
            .unwrap()
            .unwrap()
            .code;
        let stale_challenge = mfa
            .create_challenge("u-stale", pending(), Duration::from_millis(1))
            .await
            .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let before = keys(&storage_c).await;
        assert_eq!(
            before.iter().filter(|k| k.starts_with("tos:code:")).count(),
            2,
            "live code and unpresented stale code: {before:?}"
        );

        let removed = storage_c.sweep_expired().await.unwrap();
        assert_eq!(removed, 2, "the stale code and the stale challenge");

        let after = keys(&storage_c).await;
        assert!(!after.iter().any(|k| k.contains("stale")));
        assert_eq!(
            after.len(),
            2 + 1,
            "live code, live challenge, versions: {after:?}"
        );

        // Live entries still work; the swept ones are gone.
        assert!(tos.take_code(&live_code).await.unwrap().is_some());
        assert!(mfa.take_challenge(&live_challenge).await.unwrap().is_some());
        assert!(tos.take_code(&stale_code).await.unwrap().is_none());
        assert!(mfa
            .take_challenge(&stale_challenge)
            .await
            .unwrap()
            .is_none());
    }
    .block();
}
