//! # Valtron runtime bootstrap (spec-43 — multi-threaded executor)
//!
//! WHY: The `foundation_netio` WebSocket client schedules its read/write task on
//! the valtron executor (`execute(task, …)`). Under the `multi` feature that work
//! runs on a global background thread pool — but the pool must be INITIALIZED and
//! its [`PoolGuard`] held alive. We want all CPU threads driving browser sockets,
//! so we seed the pool with `available_parallelism()` workers once per process.
//!
//! WHAT: [`ensure_runtime`] — idempotent, process-global init of the valtron
//! multi-threaded pool, with an optional RNG seed (for future deterministic
//! scheduling/RNG behavior).
//!
//! HOW: A `OnceLock<PoolGuard>` holds the guard for the lifetime of the test
//! process (never dropped → the pool stays up across all parallel tests). The
//! first driver/engine to connect initializes it; because the pool is
//! process-global, the FIRST call's `seed` wins — later calls are no-ops.

use std::sync::OnceLock;
use std::thread::available_parallelism;

use foundation_core::valtron::PoolGuard;

static POOL: OnceLock<PoolGuard> = OnceLock::new();

/// Ensure the valtron multi-threaded pool is running on all CPU threads.
///
/// `seed` is the RNG seed handed to `initialize_pool` (`None` → `0`). One day we
/// will want deterministic scheduling/RNG; threading the seed now means that's a
/// value change, not an API change. Idempotent + process-global: first-call-wins.
pub fn ensure_runtime(seed: Option<u64>) {
    POOL.get_or_init(|| {
        let threads = available_parallelism().map_or(4, std::num::NonZeroUsize::get);
        tracing::debug!(threads, seed = seed.unwrap_or(0), "initializing valtron multi-threaded pool");
        // Some(threads) = use all CPU threads.
        foundation_core::valtron::initialize_pool(seed.unwrap_or(0), Some(threads))
    });
}
