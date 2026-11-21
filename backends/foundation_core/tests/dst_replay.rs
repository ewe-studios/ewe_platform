//! A DST run repeats from one seed (spec 84 T6).
//!
//! The full trace, IDs and wait deadlines, must be equal on a second run. The
//! two-process form of the same law lives in
//! `foundation_testing/tests/dst_replay.rs`.

use std::sync::Arc;

use foundation_compact::{Duration, Instant, SystemTime, UNIX_EPOCH};
use foundation_context::{Clock, Ctx};
use foundation_core::env;

/// The number of IDs in one run.
const IDS: usize = 100;
/// The number of waits in one run.
const WAITS: usize = 10;

/// A clock that a test moves by hand.
///
/// It is local to this test, so the harness needs no `test-clock` feature.
struct ManualClock {
    now: Instant,
    elapsed: Duration,
    wall: SystemTime,
}

impl ManualClock {
    fn new() -> Self {
        Self {
            now: Instant::now(),
            elapsed: Duration::ZERO,
            wall: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        }
    }

    fn advance(&mut self, by: Duration) {
        self.elapsed += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.now + self.elapsed
    }

    fn elapsed(&self) -> Duration {
        self.elapsed
    }

    fn wall_now(&self) -> SystemTime {
        self.wall + self.elapsed
    }
}

/// Run one full trace. The same seed must give the same lines.
fn trace(seed: u64) -> Vec<String> {
    let clock = Arc::new(std::sync::Mutex::new(ManualClock::new()));
    // The Ctx keeps its own `Arc<dyn Clock>`; the test keeps one too, to move
    // time. The two point at one allocation.
    let clock_handle: Arc<dyn Clock> = Arc::new(SharedClock(Arc::clone(&clock)));
    let ctx = Ctx::fixed(clock_handle);
    let env = env::simulated(&ctx, seed).expect("a seeded env never fails");
    ctx.store(env.clone());

    let ids = env.ids();
    let mut lines = Vec::new();
    let step = IDS / WAITS;
    for index in 0..IDS {
        lines.push(format!("id {}", ids.next_id()));
        if index % step == 0 {
            {
                let mut clock = clock.lock().expect("manual clock");
                clock.advance(Duration::from_millis(10));
            }
            let deadline = ctx.clock().now() + Duration::from_secs(1);
            lines.push(format!(
                "wait at {} until +{}ms",
                ctx.clock().elapsed().as_millis(),
                (deadline - ctx.clock().now()).as_millis()
            ));
        }
    }
    lines
}

/// A `Clock` that reads a shared, test-controlled clock.
struct SharedClock(Arc<std::sync::Mutex<ManualClock>>);

impl Clock for SharedClock {
    fn now(&self) -> Instant {
        self.0.lock().expect("manual clock").now()
    }

    fn elapsed(&self) -> Duration {
        self.0.lock().expect("manual clock").elapsed()
    }

    fn wall_now(&self) -> SystemTime {
        self.0.lock().expect("manual clock").wall_now()
    }
}

#[test]
fn dst_trace_repeats() {
    let first = trace(7);
    let second = trace(7);
    assert_eq!(first.len(), IDS + WAITS);
    assert_eq!(first, second, "one seed must replay the run exactly");
    assert_ne!(first, trace(8), "a different seed gives a different run");
}
