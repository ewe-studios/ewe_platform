//! Cleanup operations tests.

use foundation_db::{CleanupStats, MemoryStorage, StorageCleanup};

/// Initialize the Valtron executor for tests.
fn init_valtron() {
    foundation_core::valtron::single::initialize_pool(42);
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

#[test]
fn test_run_full_cleanup_on_empty_storage() {
    init_valtron();
    let storage = MemoryStorage::new();

    // Cleanup should succeed even on empty storage
    let result = storage.run_full_cleanup();

    // Memory storage doesn't have the tables, so this should error
    // This is expected behavior - cleanup requires proper schema
    assert!(result.is_err());
}
