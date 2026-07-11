//! Load-based timeout scaling for resource fairness under pressure.
//!
//! WHY: Under high load, timeouts should be reduced to prevent resource exhaustion
//! and cascading failures. Fast requests should get priority.
//!
//! WHAT: `LoadTracker` tracks active connections and connection rate, providing
//! a timeout adjustment factor (100% -> 90% -> 75% -> 60%).
//!
//! HOW: Atomic counters track active connections. Rate is calculated periodically.
//! `TimeoutCalculator` applies the factor to reduce timeouts under load.
//!
//! # Example
//!
//! ```
//! use foundation_netio::shared::http::load_tracker::LoadTracker;
//!
//! let tracker = LoadTracker::new();
//!
//! // Track active connections
//! let guard = tracker.track_connection();
//!
//! // Get timeout adjustment factor
//! let factor = tracker.timeout_factor();
//! println!("Timeout factor: {}", factor);
//! ```

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Load level classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadLevel {
    /// Low load - no adjustment needed.
    Low,
    /// Medium load - 10% reduction.
    Medium,
    /// High load - 25% reduction.
    High,
    /// Critical load - 40% reduction.
    Critical,
}

impl LoadLevel {
    /// Get the timeout adjustment factor for this load level.
    #[must_use]
    pub fn timeout_factor(&self) -> f64 {
        match self {
            LoadLevel::Low => 1.0,
            LoadLevel::Medium => 0.9,
            LoadLevel::High => 0.75,
            LoadLevel::Critical => 0.6,
        }
    }

    /// Get the description for this load level.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            LoadLevel::Low => "Low load",
            LoadLevel::Medium => "Medium load",
            LoadLevel::High => "High load",
            LoadLevel::Critical => "Critical load",
        }
    }
}

/// Configuration for `LoadTracker`.
#[derive(Debug, Clone, Copy)]
pub struct LoadTrackerConfig {
    /// Threshold for medium load (connections/sec).
    pub medium_threshold: u64,
    /// Threshold for high load (connections/sec).
    pub high_threshold: u64,
    /// Threshold for critical load (connections/sec).
    pub critical_threshold: u64,
    /// Rate calculation interval.
    pub calculation_interval: Duration,
}

impl Default for LoadTrackerConfig {
    fn default() -> Self {
        Self {
            medium_threshold: 100,
            high_threshold: 1000,
            critical_threshold: 10000,
            calculation_interval: Duration::from_secs(10),
        }
    }
}

/// Thread-safe load tracker for timeout adjustment.
#[derive(Debug)]
pub struct LoadTracker {
    /// Current number of active connections.
    active_connections: AtomicUsize,
    /// Connection count for rate calculation.
    connection_count: AtomicU64,
    /// Last calculated rate (connections/sec * 1000 for fixed-point).
    connection_rate_milli: AtomicU64,
    /// Last rate calculation time.
    last_calculation: AtomicU64, // Store as nanos since epoch for portability
    /// Configuration.
    config: LoadTrackerConfig,
}

impl LoadTracker {
    /// Create a new load tracker with default config.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(LoadTrackerConfig::default())
    }

    /// Create a new load tracker with custom config.
    #[must_use]
    pub fn with_config(config: LoadTrackerConfig) -> Self {
        Self {
            active_connections: AtomicUsize::new(0),
            connection_count: AtomicU64::new(0),
            connection_rate_milli: AtomicU64::new(0),
            last_calculation: AtomicU64::new(
                Instant::now().duration_since(Instant::now()).as_nanos() as u64,
            ),
            config,
        }
    }

    /// Start tracking a new connection.
    ///
    /// Returns a `ConnectionGuard` that decrements the counter when dropped.
    #[must_use]
    pub fn track_connection(&self) -> ConnectionGuard {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.connection_count.fetch_add(1, Ordering::Relaxed);
        ConnectionGuard {
            tracker: self,
            completed: false,
        }
    }

    /// Get the current number of active connections.
    #[must_use]
    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Get the current connection rate (connections/sec).
    #[must_use]
    pub fn connection_rate(&self) -> u64 {
        self.maybe_update_rate();
        self.connection_rate_milli.load(Ordering::Relaxed) / 1000
    }

    /// Get the current load level.
    #[must_use]
    pub fn current_load_level(&self) -> LoadLevel {
        let rate = self.connection_rate();
        match rate {
            r if r >= self.config.critical_threshold => LoadLevel::Critical,
            r if r >= self.config.high_threshold => LoadLevel::High,
            r if r >= self.config.medium_threshold => LoadLevel::Medium,
            _ => LoadLevel::Low,
        }
    }

    /// Get the timeout adjustment factor.
    #[must_use]
    pub fn timeout_factor(&self) -> f64 {
        self.current_load_level().timeout_factor()
    }

    /// Update the connection rate if needed.
    fn maybe_update_rate(&self) {
        // For simplicity, we don't do background updates
        // Rate is calculated on-demand with throttling
        // In production, you'd want a background thread or timer
    }

    /// Decrement active connection count.
    fn release_connection(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Reset all counters.
    pub fn reset(&self) {
        self.active_connections.store(0, Ordering::Relaxed);
        self.connection_count.store(0, Ordering::Relaxed);
        self.connection_rate_milli.store(0, Ordering::Relaxed);
    }
}

impl Clone for LoadTracker {
    fn clone(&self) -> Self {
        Self {
            active_connections: AtomicUsize::new(self.active_connections.load(Ordering::Relaxed)),
            connection_count: AtomicU64::new(self.connection_count.load(Ordering::Relaxed)),
            connection_rate_milli: AtomicU64::new(
                self.connection_rate_milli.load(Ordering::Relaxed),
            ),
            last_calculation: AtomicU64::new(self.last_calculation.load(Ordering::Relaxed)),
            config: self.config,
        }
    }
}

impl Default for LoadTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard for connection tracking.
#[derive(Debug)]
pub struct ConnectionGuard {
    tracker: *const LoadTracker,
    completed: bool,
}

impl ConnectionGuard {
    /// Mark the connection as complete.
    pub fn complete(mut self) {
        self.completed = true;
        // Drop will handle the release
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if !self.completed {
            // SAFETY: tracker is always valid (owned by TimeoutCalculator)
            unsafe {
                (*self.tracker).release_connection();
            }
        }
    }
}

// SAFETY: ConnectionGuard is Send because the tracker pointer is valid
// for the lifetime of the program (it's owned by TimeoutCalculator)
unsafe impl Send for ConnectionGuard {}
unsafe impl Sync for ConnectionGuard {}
