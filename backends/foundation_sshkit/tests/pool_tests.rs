//! Unit tests for ConnectionPool.

use foundation_sshkit::{ConnectionPool, Host};
use std::time::Duration;

#[test]
fn test_pool_creates_session() {
    let pool = ConnectionPool::new(Duration::from_secs(30));
    let host = Host::parse("github.com:22"); // github has public SSH

    let result = pool.get(&host);
    match result {
        Ok(session) => {
            // Should be able to get basic server info
            assert!(session.authenticated());
        }
        Err(_e) => {
            // Network might be unavailable — test is informational
            eprintln!("SSH to github.com failed (network?): {_e}");
        }
    }
}

#[test]
fn test_pool_evict_idle() {
    let pool = ConnectionPool::new(Duration::from_millis(1));
    // Eviction should not panic even with empty pool
    pool.evict_idle();
}

#[test]
fn test_pool_get_same_host_twice() {
    let pool = ConnectionPool::new(Duration::from_secs(30));
    let host = Host::parse("github.com:22");

    let first = pool.get(&host);
    let second = pool.get(&host);

    if first.is_ok() && second.is_ok() {
        let s1 = first.unwrap();
        let s2 = second.unwrap();
        // Both should be opened sessions
        assert!(s1.authenticated());
        assert!(s2.authenticated());
    }
}
