//! Tests for the timer that [`Ctx::timer`] derives from the context clock.
//!
//! These run on native only; the wasm fallback has its own test file.

#![cfg(not(target_family = "wasm"))]

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::Duration;

use foundation_context::env::{CounterSource, Env, SeededEntropy, Timer, TimerFuture};
use foundation_context::timer::{notify_clock_advanced, pending_deadline_count, sleep_until};
use foundation_context::{Clock, Ctx, Instant, SystemTime};

/// A clock that a test moves by hand.
struct ManualClock {
    base: Instant,
    offset: Mutex<Duration>,
    /// The timer that the clock provides, when a test sets one.
    timer: Option<Arc<dyn Timer>>,
}

impl ManualClock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            base: Instant::now(),
            offset: Mutex::new(Duration::ZERO),
            timer: None,
        })
    }

    fn with_timer(timer: Arc<dyn Timer>) -> Arc<Self> {
        Arc::new(Self {
            base: Instant::now(),
            offset: Mutex::new(Duration::ZERO),
            timer: Some(timer),
        })
    }

    fn advance(&self, by: Duration) {
        *self.offset.lock().expect("the clock offset lock") += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.base + *self.offset.lock().expect("the clock offset lock")
    }

    fn elapsed(&self) -> Duration {
        *self.offset.lock().expect("the clock offset lock")
    }

    fn wall_now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + *self.offset.lock().expect("the clock offset lock")
    }

    fn timer(&self) -> Option<Arc<dyn Timer>> {
        self.timer.clone()
    }
}

fn as_clock(clock: &Arc<ManualClock>) -> Arc<dyn Clock> {
    clock.clone()
}

/// A timer that is only an identity marker: the tests compare pointers.
struct MarkerTimer;

impl Timer for MarkerTimer {
    fn sleep_until(&self, _ctx: Ctx, _deadline: Instant) -> TimerFuture {
        Box::pin(async {})
    }
}

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

/// Park until `woken` is set, with a real-time bound.
fn wait_for_wake(wake: &CountingWake) {
    let started = std::time::Instant::now();
    while !wake.woken.load(Ordering::SeqCst) {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the timer did not wake the parked future"
        );
        thread::park_timeout(Duration::from_millis(5));
    }
}

#[test]
fn env_timer_overrides_the_default() {
    let clock = ManualClock::new();
    let ctx = Ctx::fixed(as_clock(&clock));
    let marker: Arc<dyn Timer> = Arc::new(MarkerTimer);
    ctx.store(Env::new(
        Arc::clone(&marker),
        Arc::new(SeededEntropy::new(7)),
        Arc::new(CounterSource::new(1)),
    ));

    let timer = ctx.timer();
    assert!(
        Arc::ptr_eq(&timer, &marker),
        "the Env timer must win over the derived timer"
    );
}

#[test]
fn clock_timer_wins_over_the_default() {
    let marker: Arc<dyn Timer> = Arc::new(MarkerTimer);
    let clock = ManualClock::with_timer(Arc::clone(&marker));
    let ctx = Ctx::fixed(as_clock(&clock));

    let timer = ctx.timer();
    assert!(
        Arc::ptr_eq(&timer, &marker),
        "the clock timer must win over the derived timer"
    );
}

#[test]
fn ctx_timer_defaults_to_the_clock() {
    let clock = ManualClock::new();
    let ctx = Ctx::fixed(as_clock(&clock));

    // The default derives from the clock and is cached.
    let first = ctx.timer();
    let second = ctx.timer();
    assert!(
        Arc::ptr_eq(&first, &second),
        "the derived timer must be cached on the context"
    );

    // It waits on the context clock: the simulated advance wakes the parked
    // future with no real wait.
    let wake = Arc::new(CountingWake {
        woken: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut cx = Context::from_waker(&waker);
    let deadline = clock.now() + Duration::from_secs(3_600);
    let mut sleep = first.sleep_until(ctx.clone(), deadline);
    assert!(sleep.as_mut().poll(&mut cx).is_pending());

    clock.advance(Duration::from_secs(3_600));
    notify_clock_advanced();
    wait_for_wake(&wake);

    assert_eq!(
        sleep.as_mut().poll(&mut cx),
        Poll::Ready(()),
        "the derived timer must complete once the clock reaches the deadline"
    );
}

#[test]
fn dropped_sleep_leaves_no_entry() {
    let clock = ManualClock::new();
    let ctx = Ctx::fixed(as_clock(&clock));
    let wake = Arc::new(CountingWake {
        woken: AtomicBool::new(false),
    });
    let waker = Waker::from(wake);
    let mut cx = Context::from_waker(&waker);
    let deadline = clock.now() + Duration::from_secs(3_600);

    {
        let mut sleep = Box::pin(sleep_until(deadline, &ctx));
        assert!(sleep.as_mut().poll(&mut cx).is_pending());
        assert_eq!(
            pending_deadline_count(&as_clock(&clock)),
            1,
            "the polled sleep must register one deadline"
        );
    }

    assert_eq!(
        pending_deadline_count(&as_clock(&clock)),
        0,
        "a dropped sleep must remove its deadline"
    );
}
