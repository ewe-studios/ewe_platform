//! Cleanup operations tests.

use std::time::Duration;

use foundation_core::valtron::valtron_test;
use foundation_db::{
    AsyncQueryStore, AsyncStorageCleanup, CleanupConfig, CleanupStats, DataValue, MemoryStorage,
    TursoStorage,
};

/// A migrated Turso store in a temp dir (the auth tables cleanup targets).
async fn turso() -> (TursoStorage, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let url = dir.path().join("cleanup.db").to_string_lossy().into_owned();
    let store = TursoStorage::new(&url).await.unwrap();
    store.init_schema().await.unwrap();
    (store, dir)
}

async fn insert_jwt(store: &TursoStorage, id: &str, expires_at: i64) {
    store
        .execute_async(
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
        .query_async("SELECT id FROM jwt_tokens ORDER BY id", &[])
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
    assert!(storage.run_full_cleanup(&CleanupConfig::default()).await.is_err());
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
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .unwrap();
    // Expired 2 hours ago, and expired 2 days ago.
    insert_jwt(&store, "recent", now - 2 * 60 * 60).await;
    insert_jwt(&store, "stale", now - 2 * 24 * 60 * 60).await;

    // Default retention (30 days) keeps both.
    let stats = store.run_full_cleanup(&CleanupConfig::default()).await.unwrap();
    assert_eq!(stats.jwt_tokens_deleted, 0);

    // A 1-day retention removes only the one that expired 2 days ago.
    let config = CleanupConfig::default().with_jwt_token_retention(Duration::from_secs(24 * 60 * 60));
    let stats = store.run_full_cleanup(&config).await.unwrap();
    assert_eq!(stats.jwt_tokens_deleted, 1);
    assert_eq!(jwt_ids(&store).await, vec!["recent".to_string()]);
}

#[test]
fn cleanup_config_defaults_are_the_documented_values() {
    let c = CleanupConfig::default();
    assert_eq!(c.session_max_inactive, Duration::from_secs(86_400));
    assert_eq!(c.jwt_token_retention, Duration::from_secs(2_592_000));
    assert_eq!(c.verification_token_retention, Duration::from_secs(3_600));
    assert_eq!(c.oauth_state_retention, Duration::from_secs(600));
    assert_eq!(c.magic_link_retention, Duration::from_secs(3_600));
    assert_eq!(c.email_otp_retention, Duration::from_secs(600));
    assert_eq!(c.rate_limit_window, Duration::from_secs(3_600));
}
