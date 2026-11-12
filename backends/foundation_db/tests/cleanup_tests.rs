//! Cleanup operations tests.

use std::time::Duration;

use std::sync::Arc;

use foundation_core::valtron::{valtron_test, SimulatedClock};

use foundation_context::Ctx;
use foundation_db::{
    CleanupConfig, CleanupStats, DataValue, MemoryStorage, QueryStore, StorageCleanup, TursoStorage,
};

/// A fixed simulated wall time: 2026-01-01T00:00:00Z, in unix seconds.
const FIXED_NOW_SECS: u64 = 1_767_225_600;

/// A context over a simulated clock parked at [`FIXED_NOW_SECS`].
fn fixed_ctx() -> Ctx {
    let clock = SimulatedClock::new(Duration::ZERO);
    clock.advance_wall(Duration::from_secs(FIXED_NOW_SECS));
    Ctx::fixed(Arc::new(clock))
}

/// A migrated Turso store in a temp dir (the auth tables cleanup targets).
async fn turso() -> (TursoStorage, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let url = dir.path().join("cleanup.db").to_string_lossy().into_owned();
    let store = TursoStorage::new(&url, foundation_context::Ctx::host())
        .await
        .unwrap();
    store.init_schema().await.unwrap();
    (store, dir)
}

async fn insert_jwt(store: &TursoStorage, id: &str, expires_at: i64) {
    store
        .execute(
            "INSERT INTO jwt_tokens (id, access_token_encrypted, expires_at) VALUES (?, ?, ?)",
            &[
                DataValue::Text(id.into()),
                DataValue::Text(format!("enc-{id}")),
                DataValue::Integer(expires_at),
            ],
        )
        .await
        .unwrap();
}

async fn jwt_ids(store: &TursoStorage) -> Vec<String> {
    let rows = store
        .query("SELECT id FROM jwt_tokens ORDER BY id", &[])
        .await
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    rows.iter().map(|r| r.get::<String>(0).unwrap()).collect()
}

#[test]
fn test_cleanup_stats_total() {
    let stats = CleanupStats {
        sessions_deleted: 5,
        jwt_tokens_deleted: 10,
        verification_tokens_deleted: 3,
        oauth_states_deleted: 2,
        magic_links_deleted: 4,
        email_otps_deleted: 6,
        rate_limits_deleted: 8,
    };

    assert_eq!(stats.total_deleted(), 38);
}

#[test]
fn test_cleanup_stats_summary() {
    let stats = CleanupStats {
        sessions_deleted: 1,
        jwt_tokens_deleted: 2,
        verification_tokens_deleted: 3,
        oauth_states_deleted: 4,
        magic_links_deleted: 5,
        email_otps_deleted: 6,
        rate_limits_deleted: 7,
    };

    let summary = stats.summary();
    assert!(summary.contains("1 sessions"));
    assert!(summary.contains("2 JWT tokens"));
    assert!(summary.contains("3 verification tokens"));
    assert!(summary.contains("4 OAuth states"));
    assert!(summary.contains("5 magic links"));
    assert!(summary.contains("6 email OTPs"));
    assert!(summary.contains("7 rate limits"));
}

#[test]
fn test_cleanup_stats_default() {
    let stats = CleanupStats::default();
    assert_eq!(stats.total_deleted(), 0);
}

#[valtron_test]
async fn test_run_full_cleanup_without_the_tables_is_an_error() {
    // The in-memory store has no auth tables, so the cleanup queries fail.
    let storage = MemoryStorage::new();
    assert!(storage
        .run_full_cleanup(&CleanupConfig::default(), &fixed_ctx())
        .await
        .is_err());
}

#[valtron_test]
async fn cleanup_expired_jwt_tokens_removes_only_expired_rows() {
    let (store, _dir) = turso().await;
    insert_jwt(&store, "old", 10).await;
    insert_jwt(&store, "new", i64::MAX).await;

    assert_eq!(store.cleanup_expired_jwt_tokens(100).await.unwrap(), 1);
    assert_eq!(jwt_ids(&store).await, vec!["new".to_string()]);
}

#[valtron_test]
async fn run_full_cleanup_honours_the_configured_retention() {
    let (store, _dir) = turso().await;
    let ctx = fixed_ctx();
    let now = i64::try_from(FIXED_NOW_SECS * 1_000).unwrap();
    // Expired 2 hours ago, and expired 2 days ago (both in milliseconds).
    insert_jwt(&store, "recent", now - 2 * 60 * 60 * 1000).await;
    insert_jwt(&store, "stale", now - 2 * 24 * 60 * 60 * 1000).await;

    // Default retention (30 days) keeps both.
    let stats = store
        .run_full_cleanup(&CleanupConfig::default(), &ctx)
        .await
        .unwrap();
    assert_eq!(stats.jwt_tokens_deleted, 0);

    // A 1-day retention removes only the one that expired 2 days ago.
    let config = CleanupConfig::default().with_jwt_token_retention(Duration::from_hours(24));
    let stats = store.run_full_cleanup(&config, &ctx).await.unwrap();
    assert_eq!(stats.jwt_tokens_deleted, 1);
    assert_eq!(jwt_ids(&store).await, vec!["recent".to_string()]);
}

#[valtron_test]
async fn cleanup_deletes_expired_rows_by_documented_unit() {
    let (store, _dir) = turso().await;
    let now_ms = i64::try_from(FIXED_NOW_SECS * 1_000).unwrap();
    // One millisecond before the cutoff is expired; one millisecond after is
    // not. A seconds-based comparison would treat both as the same instant.
    insert_jwt(&store, "expired", now_ms - 1).await;
    insert_jwt(&store, "kept", now_ms + 1).await;

    assert_eq!(store.cleanup_expired_jwt_tokens(now_ms).await.unwrap(), 1);
    assert_eq!(jwt_ids(&store).await, vec!["kept".to_string()]);
}

#[test]
fn cleanup_config_defaults_are_the_documented_values() {
    let c = CleanupConfig::default();
    assert_eq!(c.session_max_inactive, Duration::from_hours(24));
    assert_eq!(c.jwt_token_retention, Duration::from_hours(720));
    assert_eq!(c.verification_token_retention, Duration::from_secs(3_600));
    assert_eq!(c.oauth_state_retention, Duration::from_secs(600));
    assert_eq!(c.magic_link_retention, Duration::from_secs(3_600));
    assert_eq!(c.email_otp_retention, Duration::from_secs(600));
    assert_eq!(c.rate_limit_window, Duration::from_secs(3_600));
}
