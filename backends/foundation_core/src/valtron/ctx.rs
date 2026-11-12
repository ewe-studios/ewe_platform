//! Test clock and the shared context re-exports.
//!
//! `Ctx`, `Clock`, `MonotonicClock`, `Config` and `MinimumElement` live in the
//! `foundation_context` crate. This module re-exports them, so existing paths keep
//! working. `SimulatedClock` stays here: its `advance` wakes the parked executors
//! through a crate-private function.

#[cfg(feature = "test-clock")]
use foundation_compact::{Duration, Instant, SystemTime};

#[cfg(feature = "test-clock")]
use crate::compati::Mutex;

pub use foundation_context::{Clock, Config, Ctx, MinimumElement, MonotonicClock};

/// A [`Clock`] that a test moves forward by hand, behind the `test-clock`
/// feature.
///
/// The clock stores a base [`Instant`] captured when you build it and a
/// monotonic offset that only [`SimulatedClock::advance`] changes.
/// [`Clock::now`] returns the base plus the offset, so two reads differ by
/// exactly the time the test advanced. The absolute value is a monotonic instant
/// shifted by the offset, not a calendar epoch.
///
/// The wall clock starts at the UNIX epoch and moves only through
/// [`SimulatedClock::advance_wall`]. It never reads the real wall clock, so a
/// test is deterministic. The clock is `Send + Sync`, so a test can read it from
/// any thread.
#[cfg(feature = "test-clock")]
#[derive(Debug)]
pub struct SimulatedClock {
    base: Instant,
    offset: Mutex<Duration>,
    wall_origin: SystemTime,
    wall_offset: Mutex<Duration>,
}

#[cfg(feature = "test-clock")]
impl SimulatedClock {
    /// A clock whose monotonic offset starts at `origin` and whose wall clock
    /// starts at the UNIX epoch.
    #[must_use]
    pub fn new(origin: Duration) -> Self {
        Self {
            base: Instant::now(),
            offset: Mutex::new(origin),
            wall_origin: SystemTime::UNIX_EPOCH,
            wall_offset: Mutex::new(Duration::ZERO),
        }
    }

    /// Add `by` to the monotonic offset.
    ///
    /// WHY the wake: a parked executor blocks its thread with an operating
    /// system wait. That wait ends on a real timeout, or when a wake arrives.
    /// This clock never moves on its own, so the test that moves it must also
    /// wake the parked executors. Otherwise a task that waits for a simulated
    /// deadline sleeps on real time (GitHub issue #305). The wake goes through
    /// the same path as a readiness signal, so the executor re-reads this clock
    /// at once.
    ///
    /// # Panics
    ///
    /// Panics if the internal offset mutex is poisoned.
    pub fn advance(&self, by: Duration) {
        {
            let mut offset = self.offset.lock().expect("SimulatedClock poisoned");
            *offset += by;
        }
        // Release the lock first: a wake listener may read this clock, and
        // `now` takes the same lock.
        crate::valtron::executors::request_executor_resume();
        // A timer future that waits on this clock must look again too.
        #[cfg(all(feature = "std", not(target_family = "wasm")))]
        crate::valtron::timer::nudge();
    }

    /// Add `by` to the wall-clock offset.
    ///
    /// It wakes parked executors for the same reason as [`Self::advance`]: a
    /// caller may wait for a wall-clock deadline that this time change meets.
    ///
    /// # Panics
    ///
    /// Panics if the internal wall-offset mutex is poisoned.
    pub fn advance_wall(&self, by: Duration) {
        {
            let mut offset = self.wall_offset.lock().expect("SimulatedClock poisoned");
            *offset += by;
        }
        crate::valtron::executors::request_executor_resume();
        // A timer future that waits on this clock must look again too.
        #[cfg(all(feature = "std", not(target_family = "wasm")))]
        crate::valtron::timer::nudge();
    }
}

#[cfg(feature = "test-clock")]
impl Clock for SimulatedClock {
    fn now(&self) -> Instant {
        self.base + *self.offset.lock().expect("SimulatedClock poisoned")
    }

    fn elapsed(&self) -> Duration {
        *self.offset.lock().expect("SimulatedClock poisoned")
    }

    fn wall_now(&self) -> SystemTime {
        self.wall_origin + *self.wall_offset.lock().expect("SimulatedClock poisoned")
    }
}

/// The engine root that a seed-taking pool constructor builds.
///
/// WHY: a pool constructor such as `initialize_pool(seed, ..)` takes a seed so
/// that a run can repeat. The root [`Ctx`] must draw from that seed too.
/// `Ctx::host` would seed the task RNG from the entropy source and ignore the
/// pool seed.
/// WHAT: [`Ctx::with_seed`] with the real [`MonotonicClock`] and the pool seed.
/// The constructors are host entry points, so the real clock is correct here.
/// A deterministic simulation test passes its own root to the `*_with_ctx`
/// constructors instead.
pub(crate) fn seeded_root(seed: u64) -> Ctx {
    Ctx::with_seed(std::sync::Arc::new(MonotonicClock::new()), seed)
}
