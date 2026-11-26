//! The trusted-client store over `foundation_db`'s SQL storage (SQLite via
//! Turso): persistence, revocation, last-seen and single-use secrets.

// The `#[valtron_test]` `Send` check descends a deep dependency chain.
#![recursion_limit = "512"]

use std::sync::Arc;

use foundation_agent_rpc::trust::{
    SecretUse, SqlTrustStore, TrustStore, TrustedClient, UseOutcome,
};
use foundation_core::valtron::valtron_test;
use futures::StreamExt as _;

fn client(fp: &str, name: &str, enrolled_at: u64) -> TrustedClient {
    TrustedClient {
        fingerprint_sha256: fp.into(),
        client_id: None,
        client_name: name.into(),
        enrolled_at,
        expires_at: enrolled_at + 1000,
        last_seen_at: None,
        revoked_at: None,
    }
}

/// The host's side-effect bundle. The store only needs it for its connection
/// gate, and a test runs on the real host.
fn ctx() -> foundation_context::Ctx {
    foundation_context::Ctx::host()
}

async fn open(path: &std::path::Path) -> Arc<dyn TrustStore> {
    Arc::new(SqlTrustStore::open_sqlite(path, ctx()).await.expect("open"))
}

async fn list(store: &dyn TrustStore) -> Vec<TrustedClient> {
    let mut stream = store.clients().await.expect("clients");
    let mut clients = Vec::new();
    while let Some(client) = stream.next().await {
        clients.push(client.expect("client"));
    }
    clients
}

#[valtron_test(threads = 4)]
async fn clients_persist_across_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested").join("t.db");
    {
        let store = open(&path).await;
        store.init().await.expect("init");
        assert_eq!(list(store.as_ref()).await, Vec::new());
        store
            .record_enrollment(&client("aa", "laptop", 100), "PEM-A")
            .await
            .expect("record");
        store
            .record_enrollment(&client("bb", "phone", 200), "PEM-B")
            .await
            .expect("record");
        store.mark_seen("aa", 150).await.expect("seen");
        assert!(store.revoke_client("bb", 250).await.expect("revoke"));
        assert!(
            !store.revoke_client("bb", 260).await.expect("revoke again"),
            "already revoked"
        );
        assert!(!store.revoke_client("zz", 260).await.expect("unknown"));
    }

    let reopened = open(&path).await;
    let clients = list(reopened.as_ref()).await;
    assert_eq!(clients.len(), 2);
    assert_eq!(clients[0].client_name, "laptop");
    assert_eq!(clients[0].last_seen_at, Some(150));
    assert_eq!(clients[1].revoked_at, Some(250));
    assert!(reopened
        .client("aa")
        .await
        .expect("get")
        .expect("present")
        .is_active_at(500));
    assert!(!reopened
        .client("bb")
        .await
        .expect("get")
        .expect("present")
        .is_active_at(500));
    assert!(reopened.client("zz").await.expect("get").is_none());
}

#[valtron_test(threads = 4)]
async fn a_fingerprint_is_recorded_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(&dir.path().join("t.db")).await;
    store.init().await.expect("init");
    store
        .record_enrollment(&client("aa", "laptop", 1), "PEM")
        .await
        .expect("record");
    assert!(store
        .record_enrollment(&client("aa", "impostor", 2), "PEM")
        .await
        .is_err());
    assert_eq!(
        store
            .client("aa")
            .await
            .expect("get")
            .expect("kept")
            .client_name,
        "laptop"
    );
}

fn use_of(secret_id: &str, valid_until: u64, max_uses: u32, now: u64) -> SecretUse<'_> {
    SecretUse {
        secret_id,
        valid_until,
        max_uses,
        now,
    }
}

#[valtron_test(threads = 4)]
async fn secret_uses_are_counted_and_survive_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("uses.db");
    let store = open(&path).await;
    store.init().await.expect("init");

    assert_eq!(
        store
            .consume_secret_use(use_of("s1", 1000, 1, 10))
            .await
            .expect("first"),
        UseOutcome::Counted { uses: 1 }
    );
    assert_eq!(
        store
            .consume_secret_use(use_of("s1", 1000, 1, 11))
            .await
            .expect("replay"),
        UseOutcome::Spent
    );
    assert_eq!(
        store
            .consume_secret_use(use_of("s2", 1000, 2, 10))
            .await
            .expect("1/2"),
        UseOutcome::Counted { uses: 1 }
    );
    assert_eq!(
        store
            .consume_secret_use(use_of("s2", 1000, 2, 10))
            .await
            .expect("2/2"),
        UseOutcome::Counted { uses: 2 }
    );
    assert_eq!(
        store
            .consume_secret_use(use_of("s2", 1000, 2, 10))
            .await
            .expect("3/2"),
        UseOutcome::Spent
    );
    assert_eq!(
        store
            .consume_secret_use(use_of("s3", 1000, 0, 10))
            .await
            .expect("0"),
        UseOutcome::Spent
    );
    drop(store);

    let reopened = open(&path).await;
    assert_eq!(
        reopened
            .consume_secret_use(use_of("s1", 1000, 1, 12))
            .await
            .expect("after restart"),
        UseOutcome::Spent,
        "a spent secret stays spent across a restart"
    );
    // Pruning removes the rows of expired secrets; a secret with a fresh
    // window then counts from one again.
    assert_eq!(reopened.prune(2000).await.expect("prune"), 2);
    assert_eq!(
        reopened
            .consume_secret_use(use_of("s1", 3000, 1, 10))
            .await
            .expect("fresh window"),
        UseOutcome::Counted { uses: 1 }
    );
}
