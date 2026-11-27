//! A move of the injected clock wakes a sleep that came from `Ctx::timer`.
//!
//! WHY: the default timer derives from the context clock (spec 92). A
//! `SimulatedClock` moves only when a test moves it, so a sleep on that clock
//! must learn about the move with no real wait. `SimulatedClock::advance` calls
//! `foundation_context::timer::notify_clock_advanced` for this reason.
//!
//! HOW: park a `ctx.timer()` sleep for one simulated hour, advance the clock by
//! one hour, and check that the timer thread wakes the future at once.

// The test bounds a real wait with the real clock on purpose.
#![allow(clippy::disallowed_methods)]
#![cfg(all(feature = "test-clock", not(target_family = "wasm")))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use foundation_context::{Clock, Ctx};
use foundation_core::valtron::SimulatedClock;

/// The simulated time the sleep waits for.
const ONE_HOUR: Duration = Duration::from_secs(3_600);

/// A waker that records that it was woken.
struct CountingWake {
    woken: AtomicBool,
}

impl Wake for CountingWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.woken.store(true, Ordering::SeqCst);
    }
}

#[test]
fn advancing_the_clock_wakes_a_ctx_timer_sleep() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let as_clock: Arc<dyn Clock> = clock.clone();
    let ctx = Ctx::fixed(as_clock);
    let wake = Arc::new(CountingWake {
        woken: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut cx = Context::from_waker(&waker);

    let deadline = ctx.clock().now() + ONE_HOUR;
    let mut sleep = ctx.timer().sleep_until(ctx.clone(), deadline);
    assert!(sleep.as_mut().poll(&mut cx).is_pending());

    let started = std::time::Instant::now();
    clock.advance(ONE_HOUR);
    while !wake.woken.load(Ordering::SeqCst) {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the clock advance did not wake the derived timer's sleep"
        );
        std::thread::park_timeout(Duration::from_millis(5));
    }

    assert_eq!(
        sleep.as_mut().poll(&mut cx),
        Poll::Ready(()),
        "the sleep must complete at the simulated deadline"
    );
}
