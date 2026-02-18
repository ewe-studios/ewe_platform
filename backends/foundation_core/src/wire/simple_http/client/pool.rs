//! Connection pooling for HTTP client (STUB - To be implemented).
//!
//! WHY: Connection pooling improves performance by reusing TCP connections across
//! multiple HTTP requests to the same host.
//!
//! WHAT: Will implement `ConnectionPool` for managing pooled `SharedByteBufferStream<RawStream>`
//! connections. Currently stubbed to unblock api.rs implementation.
//!
//! HOW: Will use Arc<Mutex<HashMap>> for thread-safe pooling with per-host limits
//! and stale connection cleanup.
//!
//! TODO: Full implementation pending (Phase 1 of PLAN.md)

use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::RawStream;
use std::time::Duration;

/// Connection pool for reusing HTTP connections (STUB).
///
/// WHY: Reusing connections avoids TCP handshake overhead for multiple requests
/// to the same server.
///
/// WHAT: Placeholder stub that provides the minimal API needed by `HttpRequestTask`.
/// Currently does not actually pool connections.
///
/// HOW: Will be implemented with Arc<Mutex<`HashMap`<String, `VecDeque`<PooledStream>>>>
/// for thread-safe connection management.
///
/// TODO: Implement full pooling logic per PLAN.md Phase 1
pub struct ConnectionPool {
    #[allow(dead_code)]
    max_per_host: usize,
    #[allow(dead_code)]
    max_idle_time: Duration,
}

impl ConnectionPool {
    /// Creates a new connection pool (stub).
    ///
    /// TODO: Initialize internal `HashMap` and configure limits
    ///
    /// # Arguments
    ///
    /// * `max_per_host` - Maximum connections to pool per host
    /// * `max_idle_time` - Maximum time a connection can be idle before cleanup
    #[allow(dead_code)]
    #[must_use]
    pub fn new(max_per_host: usize, max_idle_time: Duration) -> Self {
        Self {
            max_per_host,
            max_idle_time,
        }
    }

    /// Attempts to checkout a pooled connection (stub).
    ///
    /// TODO: Implement actual checkout logic - check `HashMap`, validate staleness
    ///
    /// # Arguments
    ///
    /// * `host` - Hostname to lookup
    /// * `port` - Port number
    ///
    /// # Returns
    ///
    /// `Some(stream)` if a valid pooled connection exists, `None` otherwise.
    /// Currently always returns `None` (no pooling).
    #[allow(dead_code)]
    #[must_use]
    pub fn checkout(&self, _host: &str, _port: u16) -> Option<SharedByteBufferStream<RawStream>> {
        // TODO: Implement checkout logic
        None
    }

    /// Returns a connection to the pool (stub).
    ///
    /// TODO: Implement checkin logic - add to `HashMap`, enforce limits, LRU eviction
    ///
    /// # Arguments
    ///
    /// * `host` - Hostname
    /// * `port` - Port number
    /// * `stream` - The stream to return to pool
    #[allow(dead_code)]
    pub fn checkin(&self, _host: &str, _port: u16, _stream: SharedByteBufferStream<RawStream>) {
        // TODO: Implement checkin logic
    }

    /// Cleans up stale connections (stub).
    ///
    /// TODO: Implement cleanup - iterate pools, remove entries older than `max_idle_time`
    #[allow(dead_code)]
    pub fn cleanup_stale(&self) {
        // TODO: Implement cleanup logic
    }

    /// Clears all pooled connections (stub).
    ///
    /// TODO: Implement clear - useful for testing and shutdown
    #[allow(dead_code)]
    pub fn clear(&self) {
        // TODO: Implement clear logic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: Verify ConnectionPool::new creates pool (stub test)
    /// WHAT: Tests that constructor works
    #[test]
    fn test_connection_pool_new() {
        let pool = ConnectionPool::new(10, Duration::from_secs(60));
        assert_eq!(pool.max_per_host, 10);
        assert_eq!(pool.max_idle_time, Duration::from_secs(60));
    }

    /// WHY: Verify checkout returns None for now (stub behavior)
    /// WHAT: Tests that stub doesn't crash
    #[test]
    fn test_connection_pool_checkout_stub() {
        let pool = ConnectionPool::new(10, Duration::from_secs(60));
        let result = pool.checkout("example.com", 80);
        assert!(result.is_none());
    }

    // TODO: Add real tests once implementation is complete
}
