//! The trust store: one behaviour over three backends, and revocation.

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::trust::{
    KvTrustStore, MemoryTrustStore, SecretUse, SqlTrustStore, TrustConfig, TrustStore,
    TrustedClient, TrustedClients, UseOutcome,
};
use foundation_context::Ctx;
use foundation_core::valtron::{valtron_test, SimulatedClock};
use futures_util::StreamExt as _;

fn ctx_at(secs: u64) -> Ctx {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(secs));
    let as_clock: Arc<dyn foundation_context::Clock> = clock;
    Ctx::fixed(as_clock)
}

fn client(fingerprint: &str, name: &str, enrolled_at: u64) -> TrustedClient {
    TrustedClient {
        fingerprint_sha256: fingerprint.to_string(),
        client_name: name.to_string(),
        enrolled_at,
        expires_at: 1_000,
        last_seen_at: None,
        revoked_at: None,
    }
}

async fn sql_store() -> SqlTrustStore<foundation_db::TursoStorage> {
    let storage = foundation_db::TursoStorage::open(":memory:", Ctx::host())
        .await
        .expect("in-memory turso");
    SqlTrustStore::new(Arc::new(storage), "spec87_").expect("prefix")
}

fn kv_store() -> KvTrustStore<foundation_db::MemoryStorage> {
    KvTrustStore::new(Arc::new(foundation_db::MemoryStorage::new()))
}

/// Every observable result of one fixed sequence of store operations.
// `Outcomes` compares many independent store results, and each one is a bool or
// a small enum. Splitting it would not make the comparison clearer.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, PartialEq, Eq)]
struct Outcomes {
    duplicate: bool,
    present: bool,
    revoked_first: bool,
    revoked_again: bool,
    seen: Option<u64>,
    use_one: UseOutcome,
    use_one_again: UseOutcome,
    use_two_first: UseOutcome,
    use_two_second: UseOutcome,
    use_two_third: UseOutcome,
    use_zero: UseOutcome,
    revoke_secret_first: bool,
    revoke_secret_again: bool,
    revoked_before_prune: bool,
    pruned: u64,
    revoked_after_prune: bool,
    listed: Vec<String>,
}

// The sequence is one linear script; each step depends on the step before it.
#[allow(clippy::too_many_lines)]
async fn run_sequence(store: Arc<dyn TrustStore>) -> Outcomes {
    store.init().await.expect("init");
    let a = client("aa", "laptop", 10);
    let b = client("bb", "phone", 20);
    store
        .record_enrollment(&a, "cert-a")
        .await
        .expect("enroll a");
    store
        .record_enrollment(&b, "cert-b")
        .await
        .expect("enroll b");
    let duplicate = store.record_enrollment(&a, "cert-a").await.is_err();
    let present = store.client("aa").await.expect("get").is_some();
    let revoked_first = store.revoke_client("aa", 100).await.expect("revoke");
    let revoked_again = store.revoke_client("aa", 110).await.expect("revoke");
    store.mark_seen("bb", 150).await.expect("seen");
    let seen = store
        .client("bb")
        .await
        .expect("get")
        .and_then(|record| record.last_seen_at);

    let use_one = store
        .consume_secret_use(SecretUse {
            secret_id: "s1",
            valid_until: 1_000,
            max_uses: 1,
            now: 10,
        })
        .await
        .expect("use");
    let use_one_again = store
        .consume_secret_use(SecretUse {
            secret_id: "s1",
            valid_until: 1_000,
            max_uses: 1,
            now: 11,
        })
        .await
        .expect("use");
    let use_two_first = store
        .consume_secret_use(SecretUse {
            secret_id: "s2",
            valid_until: 1_000,
            max_uses: 2,
            now: 10,
        })
        .await
        .expect("use");
    let use_two_second = store
        .consume_secret_use(SecretUse {
            secret_id: "s2",
            valid_until: 1_000,
            max_uses: 2,
            now: 10,
        })
        .await
        .expect("use");
    let use_two_third = store
        .consume_secret_use(SecretUse {
            secret_id: "s2",
            valid_until: 1_000,
            max_uses: 2,
            now: 10,
        })
        .await
        .expect("use");
    let use_zero = store
        .consume_secret_use(SecretUse {
            secret_id: "s3",
            valid_until: 1_000,
            max_uses: 0,
            now: 10,
        })
        .await
        .expect("use");
    let revoke_secret_first = store
        .revoke_secret("s1", 1_000, 20)
        .await
        .expect("revoke secret");
    let revoke_secret_again = store
        .revoke_secret("s1", 1_000, 21)
        .await
        .expect("revoke secret");
    let revoked_before_prune = store.is_secret_revoked("s1").await.expect("is revoked");
    let pruned = store.prune(2_000).await.expect("prune");
    let revoked_after_prune = store.is_secret_revoked("s1").await.expect("is revoked");

    let mut stream = store.clients().await.expect("clients");
    let mut listed = Vec::new();
    while let Some(record) = stream.next().await {
        listed.push(record.expect("client").fingerprint_sha256);
    }

    Outcomes {
        duplicate,
        present,
        revoked_first,
        revoked_again,
        seen,
        use_one,
        use_one_again,
        use_two_first,
        use_two_second,
        use_two_third,
        use_zero,
        revoke_secret_first,
        revoke_secret_again,
        revoked_before_prune,
        pruned,
        revoked_after_prune,
        listed,
    }
}

#[valtron_test]
async fn trust_store_same_behaviour() {
    let memory: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
    let sql: Arc<dyn TrustStore> = Arc::new(sql_store().await);
    let kv: Arc<dyn TrustStore> = Arc::new(kv_store());

    let memory_outcomes = run_sequence(memory).await;
    let sql_outcomes = run_sequence(sql).await;
    let kv_outcomes = run_sequence(kv).await;

    assert_eq!(memory_outcomes, sql_outcomes, "memory and SQL agree");
    assert_eq!(memory_outcomes, kv_outcomes, "memory and key-value agree");
    assert_eq!(
        memory_outcomes.use_one,
        UseOutcome::Counted { uses: 1 },
        "one free use"
    );
    assert_eq!(memory_outcomes.use_one_again, UseOutcome::Spent);
    assert_eq!(
        memory_outcomes.use_two_second,
        UseOutcome::Counted { uses: 2 }
    );
    assert_eq!(memory_outcomes.use_two_third, UseOutcome::Spent);
    assert!(memory_outcomes.duplicate, "a second enrollment is refused");
    assert_eq!(memory_outcomes.seen, Some(150));
    assert!(memory_outcomes.revoked_before_prune);
    assert_eq!(memory_outcomes.pruned, 3, "two use rows and one secret row");
    assert!(!memory_outcomes.revoked_after_prune);
}

#[valtron_test]
async fn revoked_not_listed() {
    let ctx = ctx_at(1_700_000_000);
    let store: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
    let trusted = TrustedClients::open(Arc::clone(&store), &ctx, TrustConfig::default())
        .await
        .expect("open");

    let mut enrolled = client("aa", "laptop", 10);
    enrolled.expires_at = 2_000_000_000;
    trusted
        .record_enrollment(enrolled, "pem")
        .await
        .expect("enroll");
    let before = trusted.get("aa").await.expect("get").expect("present");
    assert!(before.is_active_at(1_700_000_000));

    assert!(trusted.revoke_client("aa").await.expect("revoke"));
    let after = trusted.get("aa").await.expect("get").expect("record");
    assert!(after.revoked_at.is_some());
    assert!(
        !after.is_active_at(1_700_000_000),
        "a revoked client is not listed as active"
    );
    assert!(
        trusted
            .index()
            .get("aa")
            .is_some_and(|client| !client.is_active_at(1_700_000_000)),
        "the verifier index refuses the client"
    );

    // A restart reads the revocation from the store.
    let reopened = TrustedClients::open(store, &ctx, TrustConfig::default())
        .await
        .expect("reopen");
    let reloaded = reopened.get("aa").await.expect("get").expect("record");
    assert!(reloaded.revoked_at.is_some());
    assert!(!reloaded.is_active_at(1_700_000_000));
}

#[valtron_test]
async fn shared_store_requires_an_atomic_counter() {
    let ctx = ctx_at(1_700_000_000);
    let store: Arc<dyn TrustStore> = Arc::new(MemoryTrustStore::new());
    let config = TrustConfig::default().with_shared_store(true);
    let result = TrustedClients::open(store, &ctx, config).await;
    assert!(
        result.is_err(),
        "a per-process counter cannot share a store"
    );
}
