//! Load-based timeout scaling for resource fairness under pressure.
//!
//! WHY: Under high load, timeouts should be reduced to prevent resource exhaustion
//! and cascading failures. Fast requests should get priority.
//!
//! WHAT: `LoadTracker` tracks active connections and connection rate, providing
//! a timeout adjustment factor (100% -> 90% -> 75% -> 60%).
//!
//! HOW: Atomic counters track active connections. The connection rate is
//! recalculated on read, over the window since the previous read.
//! `TimeoutCalculator` applies the factor to reduce timeouts under load.
//!
//! # Example
//!
//! ```
//! use foundation_netio::shared::http::load_tracker::LoadTracker;
//! use std::sync::Arc;
//!
//! let tracker = LoadTracker::new(foundation_core::valtron::Ctx::host());
//!
//! // Track active connections
//! let guard = tracker.track_connection();
//!
//! // Get timeout adjustment factor
//! let factor = tracker.timeout_factor();
//! println!("Timeout factor: {}", factor);
//! ```

use foundation_core::valtron::Ctx;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

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
            Self::Low => 1.0,
            Self::Medium => 0.9,
            Self::High => 0.75,
            Self::Critical => 0.6,
        }
    }

    /// Get the description for this load level.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Low => "Low load",
            Self::Medium => "Medium load",
            Self::High => "High load",
            Self::Critical => "Critical load",
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
    /// Shortest window between two rate updates.
    ///
    /// A read that arrives before this much time has passed since the previous
    /// update returns the previous rate unchanged. The default is
    /// `Duration::ZERO`, so every read recalculates the rate.
    pub calculation_interval: Duration,
}

impl Default for LoadTrackerConfig {
    fn default() -> Self {
        Self {
            medium_threshold: 100,
            high_threshold: 1000,
            critical_threshold: 10000,
            calculation_interval: Duration::ZERO,
        }
    }
}

/// Thread-safe load tracker for timeout adjustment.
pub struct LoadTracker {
    /// Current number of active connections.
    active_connections: AtomicUsize,
    /// Connections tracked since the last rate calculation.
    connection_count: AtomicU64,
    /// Last calculated rate, in connections per second multiplied by 1000
    /// (fixed-point). A rate of 2.5 connections/sec stores `2500`.
    connection_rate_milli: AtomicU64,
    /// Last rate calculation time, in nanoseconds on the injected clock's
    /// [`Clock::elapsed`] timeline. Set at construction, so the first window
    /// measures from the moment the tracker was built.
    last_calculation: AtomicU64,
    /// Configuration.
    config: LoadTrackerConfig,
    /// The clock that measures the rate window. Injected, never read from a
    /// global.
    ctx: Ctx,
}

impl core::fmt::Debug for LoadTracker {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LoadTracker")
            .field(
                "active_connections",
                &self.active_connections.load(Ordering::Relaxed),
            )
            .field(
                "connection_count",
                &self.connection_count.load(Ordering::Relaxed),
            )
            .field(
                "connection_rate_milli",
                &self.connection_rate_milli.load(Ordering::Relaxed),
            )
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl LoadTracker {
    /// Create a new load tracker with default config.
    ///
    /// `clock` measures the rate window. The host passes the clock from its
    /// context bundle, so a test can move time by hand.
    #[must_use]
    pub fn new(ctx: Ctx) -> Self {
        Self::with_config(LoadTrackerConfig::default(), ctx)
    }

    /// Create a new load tracker with custom config.
    #[must_use]
    pub fn with_config(config: LoadTrackerConfig, ctx: Ctx) -> Self {
        let last_calculation = Self::clock_nanos(&ctx);
        Self {
            active_connections: AtomicUsize::new(0),
            connection_count: AtomicU64::new(0),
            connection_rate_milli: AtomicU64::new(0),
            last_calculation: AtomicU64::new(last_calculation),
            config,
            ctx,
        }
    }

    /// Start tracking a new connection.
    ///
    /// Returns a `ConnectionGuard` that decrements the counter when dropped.
    /// The guard borrows the tracker, so it cannot outlive it.
    #[must_use]
    pub fn track_connection(&self) -> ConnectionGuard<'_> {
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
    ///
    /// Recalculates the rate over the window since the previous read, so the
    /// value responds to new connections. A read inside the configured
    /// [`LoadTrackerConfig::calculation_interval`], including one with zero
    /// elapsed time, returns the previous rate.
    #[must_use]
    pub fn connection_rate(&self) -> u64 {
        self.update_rate();
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

    /// Update the connection rate from the window since the last update.
    ///
    /// One thread claims the window with a compare-and-swap on
    /// `last_calculation`. Other threads return at once and read the rate the
    /// winner stores, so the count and the elapsed time always describe the
    /// same window.
    fn update_rate(&self) {
        let now = Self::clock_nanos(&self.ctx);
        let last = self.last_calculation.load(Ordering::Relaxed);
        let elapsed_nanos = now.saturating_sub(last);
        let min_window_nanos =
            u64::try_from(self.config.calculation_interval.as_nanos()).unwrap_or(u64::MAX);

        // A zero-length (or too short) window keeps the previous rate. This is
        // the division-by-zero guard as well as the throttle.
        if elapsed_nanos == 0 || elapsed_nanos < min_window_nanos {
            return;
        }

        if self
            .last_calculation
            .compare_exchange(last, now, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        let connections = self.connection_count.swap(0, Ordering::AcqRel);
        // connections/sec * 1000 in exact integer math. `u128` holds
        // `u64::MAX * 1000 * 1e9` without overflow.
        let rate_milli = u128::from(connections)
            .saturating_mul(1_000 * 1_000_000_000)
            .checked_div(u128::from(elapsed_nanos))
            .unwrap_or(0);
        let rate_milli = u64::try_from(rate_milli).unwrap_or(u64::MAX);
        self.connection_rate_milli
            .store(rate_milli, Ordering::Relaxed);
    }

    /// The clock's elapsed time in nanoseconds, saturated to `u64`.
    fn clock_nanos(ctx: &Ctx) -> u64 {
        u64::try_from(ctx.clock().elapsed().as_nanos()).unwrap_or(u64::MAX)
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
        self.last_calculation
            .store(Self::clock_nanos(&self.ctx), Ordering::Relaxed);
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
            ctx: self.ctx.clone(),
        }
    }
}

/// RAII guard for connection tracking.
///
/// Borrows the tracker, so the tracker cannot move or drop while a guard
/// exists. The old design stored a raw pointer and promised `Send` and `Sync`
/// with `unsafe impl`s; that let safe code drop the tracker and then
/// dereference the freed pointer when the guard dropped.
#[derive(Debug)]
pub struct ConnectionGuard<'a> {
    tracker: &'a LoadTracker,
    completed: bool,
}

impl ConnectionGuard<'_> {
    /// Mark the connection as complete.
    pub fn complete(mut self) {
        self.completed = true;
        // Drop will handle the release
    }
}

impl Drop for ConnectionGuard<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.tracker.release_connection();
        }
    }
}
