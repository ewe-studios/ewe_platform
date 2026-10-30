//! `ShutdownSignal` tests (Feature 31): a wake-capable shutdown flag.
//!
//! The flag is a plain `AtomicBool` with `OnSignal` semantics (probe /
//! idempotent `turn_on` / `turn_off`). On unix it also carries a wake link to
//! the shared reactor, so a `turn_on` wakes a thread parked in
//! `Reactor::wait_for_events` at once instead of letting it sleep out its
//! timeout. That is the enabler for the accept loop's immediate shutdown.

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_reactor::ShutdownSignal;

#[cfg(unix)]
use foundation_reactor::Reactor;

/// A fresh signal starts off.
#[test]
fn starts_off() {
    let signal = ShutdownSignal::new();
    assert!(!signal.probe());
}

/// `turn_on` sets the flag and reports the false-to-true change once.
#[test]
fn turn_on_is_idempotent() {
    let signal = ShutdownSignal::new();
    assert!(signal.turn_on(), "first turn_on changes off -> on");
    assert!(signal.probe());
    assert!(!signal.turn_on(), "second turn_on is already on");
    assert!(signal.probe());
}

/// `turn_off` clears the flag and reports the true-to-false change once.
#[test]
fn turn_off_clears() {
    let signal = ShutdownSignal::new();
    assert!(signal.turn_on());
    assert!(signal.turn_off(), "turn_off changes on -> off");
    assert!(!signal.probe());
    assert!(!signal.turn_off(), "second turn_off is already off");
}

/// `register_wake` links the signal to the shared reactor, and `turn_on` wakes
/// a thread parked in `Reactor::wait_for_events` at once (no timeout). The
/// parked thread observes an advanced generation, so its lost-wakeup guard
/// re-checks its own condition instead of sleeping.
#[cfg(unix)]
#[test]
fn turn_on_wakes_a_parked_waiter() {
    let signal = Arc::new(ShutdownSignal::new());
    signal.register_wake().expect("register wake");

    // `register_wake` initialised the process-global reactor, so `get` returns
    // the same instance the signal will wake.
    let reactor = Reactor::get().expect("reactor");
    let since = reactor.events_generation();

    let parked_reactor = Arc::clone(&reactor);
    let parked = std::thread::spawn(move || {
        let t0 = Instant::now();
        let observed = parked_reactor.wait_for_events(since, Duration::from_secs(10));
        (observed, t0.elapsed())
    });

    // Give the thread a moment to reach the condvar wait before signalling.
    std::thread::sleep(Duration::from_millis(100));

    assert!(signal.turn_on(), "signal changes off -> on");
    let (observed, park_elapsed) = parked.join().expect("parked thread");

    assert!(
        park_elapsed < Duration::from_millis(500),
        "parked thread slept {park_elapsed:?}; it should wake at once, not sleep out 10 s"
    );
    assert_ne!(observed, since, "generation must advance past `since` on wake");
}
