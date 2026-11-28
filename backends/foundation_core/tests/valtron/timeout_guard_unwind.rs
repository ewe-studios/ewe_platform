//! Regression tests for issue #1088.
//!
//! A failing assertion inside a `#[foundation_macros::timeout]` test that holds
//! a valtron `PoolGuard` must report a normal panic. If the guard's drop panics
//! while the assertion unwinds, Rust turns the second panic into a process
//! abort: `panic in a destructor during cleanup` on LLVM, or
//! `fatal runtime error: failed to initiate panic` on cranelift.
//!
//! The drop path is not free of panics on its own. It reads the injected clock
//! to compute the shutdown deadline, signals the lifecycle latches, and joins
//! worker threads, and any of those can panic. A test cannot wait for a rare
//! poisoned lock, so the first test injects a clock that fails on demand. That
//! failure is deterministic and stands for every other panic in the teardown.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use foundation_context::{Clock, Ctx, MonotonicClock};
use foundation_core::valtron::{initialize_pool, initialize_pool_with_ctx};

/// A clock that panics on [`Clock::now`] once the test arms it.
///
/// WHY: `PoolGuard::shutdown` reads the clock for its deadline. A failing
/// clock is a deterministic way to make a teardown dependency panic exactly
/// when a test body unwinds (CLAUDE.md rule 11 keeps time behind the clock, so
/// this is a supported injection point).
/// WHAT: before `arm()` it reports a fixed origin; after `arm()` every `now()`
/// panics.
struct TripClock {
    armed: Arc<AtomicBool>,
    real: MonotonicClock,
}

impl TripClock {
    /// Build the clock and the flag that arms it.
    fn new() -> (Arc<Self>, Arc<AtomicBool>) {
        let armed = Arc::new(AtomicBool::new(false));
        let clock = Arc::new(Self {
            armed: Arc::clone(&armed),
            real: MonotonicClock::new(),
        });
        (clock, armed)
    }
}

impl Clock for TripClock {
    fn now(&self) -> Instant {
        assert!(
            !self.armed.load(Ordering::SeqCst),
            "the injected clock failed during pool shutdown"
        );
        self.real.now()
    }

    fn elapsed(&self) -> Duration {
        Duration::ZERO
    }

    fn wall_now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH
    }
}

/// WHY: the matrix suite holds a `PoolGuard` in a `#[timeout]` test body. A
/// failed assertion used to abort the whole test binary, hiding every other
/// result, because the guard's drop panicked while the assertion unwound.
/// WHAT: a panic in the guard's teardown must not become a second panic. The
/// assertion's panic must reach the harness unchanged.
/// HOW: hold the guard, arm the clock so the shutdown deadline read panics,
/// then fail an assertion. Before the fix this test aborted the process with
/// signal 6; after it, `#[should_panic]` observes the assertion.
#[test]
#[foundation_macros::timeout(30_000)]
#[should_panic(expected = "a failing assertion must stay a normal panic")]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift cannot unwind a panic raised on a spawned thread"
)]
fn a_failing_assertion_while_holding_a_pool_guard_stays_a_normal_panic() {
    let (clock, armed) = TripClock::new();
    let _guard = initialize_pool_with_ctx(7, Some(4), Ctx::with_seed(clock, 7));
    armed.store(true, Ordering::SeqCst);
    assert_eq!(1, 2, "a failing assertion must stay a normal panic");
}

/// WHY: the fix catches a teardown panic only while the thread already
/// unwinds. The normal exit path must keep its old behaviour.
/// WHAT: a guard dropping without an active panic still shuts the pool down
/// and releases the process-wide lifecycle gate, so the next pool starts.
/// HOW: drop one guard, then initialize and drop a second pool.
#[test]
#[foundation_macros::timeout(30_000)]
fn a_guard_dropped_on_the_normal_exit_path_releases_the_pool_gate() {
    let (clock, _armed) = TripClock::new();
    drop(initialize_pool_with_ctx(
        7,
        Some(4),
        Ctx::with_seed(clock, 7),
    ));
    drop(initialize_pool(7, Some(4)));
}
