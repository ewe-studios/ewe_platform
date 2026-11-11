//! A blocking wait reads elapsed time from the injected source (spec 70
//! feature 134).
//!
//! WHY: `wait_duration` read the real monotonic clock, so a deterministic test
//! could not control when the wait ended.
//!
//! WHAT: the wait ends when the injected [`TimeSource`] reports the duration has
//! passed, without a real wait.
//!
//! HOW: the stepping source reports one hour on each read. Before this feature
//! the wait ignored the source and parked for the whole duration.

#![cfg(feature = "std")]

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use foundation_nostd::primitives::wait_duration::{wait_duration, TimeSource};

/// A source that advances one hour on every read.
struct SteppingClock {
    base: Instant,
    reads: AtomicU64,
}

impl TimeSource for SteppingClock {
    fn now(&self) -> Instant {
        let read = self.reads.fetch_add(1, Ordering::SeqCst);
        self.base + Duration::from_secs(read.saturating_mul(3_600))
    }
}

#[test]
fn wait_duration_reads_the_injected_time_source() {
    let clock = SteppingClock {
        base: Instant::now(),
        reads: AtomicU64::new(0),
    };
    let started = Instant::now();

    wait_duration(Duration::from_millis(200), &clock);

    let real = started.elapsed();
    assert!(
        real < Duration::from_millis(50),
        "the wait parked for {real:?}; it must end when the injected source reports \
         the duration has passed"
    );
    assert!(
        clock.reads.load(Ordering::SeqCst) >= 1,
        "the wait never read the injected time source"
    );
}
