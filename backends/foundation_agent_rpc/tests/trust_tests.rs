//! The trusted-client store over foundation_db's SQL storage (SQLite via
//! Turso): persistence, revocation, last-seen, single-use secrets.

use foundation_agent_rpc::trust::{TrustedClient, TrustedClientStore};
use foundation_core::valtron::valtron_test;

fn client(fp: &str, name: &str, enrolled_at: u64) -> TrustedClient {
    TrustedClient {
        fingerprint_sha256: fp.into(),
        client_name: name.into(),
        enrolled_at,
        expires_at: enrolled_at + 1000,
        last_seen_at: None,
        revoked_at: None,
    }
}

#[valtron_test(threads = 4)]
async fn clients_persist_across_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = TrustedClientStore::default_path(dir.path().join("nested"));
    {
        let store = TrustedClientStore::open_sqlite(&path).await.expect("open");
        assert!(store.list().is_empty());
        store
            .record_enrollment(client("aa", "laptop", 100), "PEM-A")
            .await
            .expect("record");
        store
            .record_enrollment(client("bb", "phone", 200), "PEM-B")
            .await
            .expect("record");
        store.mark_seen("aa", 150).await.expect("seen");
        assert!(store.revoke("bb", 250).await.expect("revoke"));
        assert!(
            !store.revoke("bb", 260).await.expect("revoke again"),
            "already revoked"
        );
        assert!(!store.revoke("zz", 260).await.expect("unknown"));
    }

    let reopened = TrustedClientStore::open_sqlite(&path).await.expect("reopen");
    let clients = reopened.list();
    assert_eq!(clients.len(), 2);
    assert_eq!(clients[0].client_name, "laptop");
    assert_eq!(clients[0].last_seen_at, Some(150));
    assert_eq!(clients[1].revoked_at, Some(250));
    assert!(reopened.is_trusted_at("aa", 500));
    assert!(!reopened.is_trusted_at("bb", 500), "revoked");
    assert!(!reopened.is_trusted_at("aa", 1100), "expired");
    assert!(!reopened.is_trusted_at("zz", 500), "unknown");
}

#[valtron_test(threads = 4)]
async fn a_fingerprint_is_recorded_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TrustedClientStore::open_sqlite(dir.path().join("t.db"))
        .await
        .expect("open");
    store
        .record_enrollment(client("aa", "laptop", 1), "PEM")
        .await
        .expect("record");
    assert!(store
        .record_enrollment(client("aa", "impostor", 2), "PEM")
        .await
        .is_err());
    assert_eq!(store.get("aa").expect("kept").client_name, "laptop");
}

#[valtron_test(threads = 4)]
async fn secret_uses_are_counted_and_survive_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("uses.db");
    let store = TrustedClientStore::open_sqlite(&path).await.expect("open");

    // Single use.
    assert!(store.consume_secret_use("s1", 1000, 1, 10).await.expect("first"));
    assert!(!store.consume_secret_use("s1", 1000, 1, 11).await.expect("replay"));
    // Two uses.
    assert!(store.consume_secret_use("s2", 1000, 2, 10).await.expect("1/2"));
    assert!(store.consume_secret_use("s2", 1000, 2, 10).await.expect("2/2"));
    assert!(!store.consume_secret_use("s2", 1000, 2, 10).await.expect("3/2"));
    // Zero uses: never.
    assert!(!store.consume_secret_use("s3", 1000, 0, 10).await.expect("0"));
    drop(store);

    let reopened = TrustedClientStore::open_sqlite(&path).await.expect("reopen");
    assert!(
        !reopened.consume_secret_use("s1", 1000, 1, 12).await.expect("after restart"),
        "a spent secret stays spent across a restart"
    );
    // Rows of expired secrets are pruned (verification refuses those secrets
    // before the store is asked, so the reset count is never reachable).
    assert!(reopened.consume_secret_use("s1", 1000, 1, 1000).await.expect("pruned"));
}
