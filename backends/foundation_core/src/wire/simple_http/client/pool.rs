//! Connection pooling for HTTP client.
//!
//! WHY: Connection pooling improves performance by reusing TCP connections across
//! multiple HTTP requests to the same host.
//!
//! WHAT: A lightweight, safe connection pool implemented with Arc<Mutex<...>>.
//! The pool stores streams per host:port and exposes `checkout`/`checkin`,
//! plus maintenance helpers `cleanup_stale` and `clear`.
//!
//! Notes:
//! - This implementation is intentionally conservative and synchronous to match
//!   the existing client module patterns. It provides a practical, testable
//!   implementation suitable for unit tests and basic reuse. Further
//!   optimizations (background cleanup task, async-aware primitives) can be
//!   added in a later phase.
//!
use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::RawStream;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Entry stored in per-host queue: (`last_used_instant`, stream)
type PooledEntry = (Instant, SharedByteBufferStream<RawStream>);

/// Connection pool for reusing HTTP connections.
///
/// WHY: Reusing connections avoids TCP handshake overhead for multiple requests
/// to the same server.
///
/// WHAT: A simple thread-safe pool keyed by `host:port` storing a `VecDeque` of
/// `PooledEntry`. The pool enforces `max_per_host` limit and expires entries
/// older than `max_idle_time` on checkout/cleanup.
pub struct ConnectionPool {
    // Max connections to retain per host
    pub max_per_host: usize,
    // Maximum idle lifetime for pooled connections
    pub max_idle_time: Duration,
    // Internal storage: host:port -> deque of (Instant, Stream)
    inner: Arc<Mutex<HashMap<String, VecDeque<PooledEntry>>>>,
}

impl std::fmt::Debug for ConnectionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Try to acquire lock to inspect the inner map. If the lock is poisoned,
        // avoid panicking and instead represent the pools as "<poisoned>".
        match self.inner.lock() {
            Ok(map) => {
                // Build a compact representation: host -> connection count
                let pools: Vec<(String, usize)> = map
                    .iter()
                    .map(|(host, q)| (host.clone(), q.len()))
                    .collect();

                f.debug_struct("ConnectionPool")
                    .field("max_per_host", &self.max_per_host)
                    .field("max_idle_time", &self.max_idle_time)
                    .field("pools", &pools)
                    .finish()
            }
            Err(_) => f
                .debug_struct("ConnectionPool")
                .field("max_per_host", &self.max_per_host)
                .field("max_idle_time", &self.max_idle_time)
                .field("pools", &"<poisoned>")
                .finish(),
        }
    }
}

const MAX_PER_HOST: usize = 10;
const MAX_IDLE_TIME: Duration = Duration::from_secs(300);

impl Default for ConnectionPool {
    fn default() -> Self {
        Self {
            max_per_host: MAX_PER_HOST,
            max_idle_time: MAX_IDLE_TIME,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl ConnectionPool {
    /// Creates a new connection pool.
    ///
    /// # Arguments
    ///
    /// * `max_per_host` - Maximum connections to pool per host
    /// * `max_idle_time` - Maximum time a connection can be idle before cleanup
    #[must_use]
    pub fn new(max_per_host: usize, max_idle_time: Duration) -> Self {
        Self {
            max_per_host,
            max_idle_time,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Attempts to checkout a pooled connection for `host:port`.
    ///
    /// Removes and returns the most-recently-added valid connection if one
    /// exists and is not stale. Otherwise returns `None`.
    #[must_use]
    pub fn checkout(&self, host: &str, port: u16) -> Option<SharedByteBufferStream<RawStream>> {
        let key = format!("{host}:{port}");
        let now = Instant::now();

        let mut map = match self.inner.lock() {
            Ok(m) => m,
            Err(_) => return None, // poisoned lock; treat as empty pool
        };

        if let Some(queue) = map.get_mut(&key) {
            // Pop from the back (LIFO reuse) until we find a valid non-stale stream
            while let Some((ts, stream)) = queue.pop_back() {
                if now.duration_since(ts) <= self.max_idle_time {
                    return Some(stream);
                }
                // else stale: continue to next
            }
            // If queue is empty, remove the key to keep map small
            if queue.is_empty() {
                map.remove(&key);
            }
        }

        None
    }

    /// Returns a connection to the pool.
    ///
    /// If the per-host buffer exceeds `max_per_host`, the oldest entry is
    /// dropped to maintain the limit.
    pub fn checkin(&self, host: &str, port: u16, stream: SharedByteBufferStream<RawStream>) {
        let key = format!("{host}:{port}");
        let mut map = match self.inner.lock() {
            Ok(m) => m,
            Err(_) => return, // poisoned lock; drop stream
        };

        let queue = map.entry(key).or_insert_with(VecDeque::new);

        // Push current time and stream to the back (newest)
        queue.push_back((Instant::now(), stream));

        // Enforce max_per_host: drop oldest while exceeding
        while queue.len() > self.max_per_host {
            queue.pop_front();
        }
    }

    /// Cleans up stale connections older than `max_idle_time`.
    ///
    /// This is safe to call periodically from a maintenance task or during
    /// heavy allocation periods.
    pub fn cleanup_stale(&self) {
        let now = Instant::now();
        let mut map = match self.inner.lock() {
            Ok(m) => m,
            Err(_) => return,
        };

        let mut remove_keys = Vec::new();

        for (key, queue) in map.iter_mut() {
            // Retain only entries that are fresh
            queue.retain(|(ts, _)| now.duration_since(*ts) <= self.max_idle_time);
            if queue.is_empty() {
                remove_keys.push(key.clone());
            }
        }

        for k in remove_keys {
            map.remove(&k);
        }
    }

    /// Clears all pooled connections (useful for tests and shutdown).
    pub fn clear(&self) {
        if let Ok(mut map) = self.inner.lock() {
            map.clear();
        }
    }
}
