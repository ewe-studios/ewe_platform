//! Issue #240: concurrent reactor initialisation must spawn exactly one drain
//! thread.
//!
//! WHY: `Reactor::init_inner` used to build a `Reactor` and spawn a
//! `reactor-drain` thread *before* racing on `REACTOR.set`. Every thread that
//! lost the race dropped its own `Reactor`, whose `Drop` could not join the
//! drain thread, so each loser left a thread polling an empty selector.
//!
//! WHAT: release several threads through a barrier so they all call
//! `Reactor::init_with_config` at once, then assert that exactly one
//! `reactor-drain` thread is alive.
//!
//! HOW: this is its own test binary because `REACTOR` is a process-wide
//! `OnceLock`: any other test in the same process could initialise the reactor
//! first and hide the race. The live thread count comes from the
//! `#[doc(hidden)]` `live_drain_threads` hook. The test requests a long drain
//! poll timeout so a loser's detached thread stays parked (a loser's `Drop`
//! sets `shutdown`, but `poll()` only observes it when the timeout expires),
//! which makes the leak observable instead of a timing accident.

#![cfg(unix)]

use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use foundation_reactor::{
    drain_thread_spawn_count, init_build_attempts, live_drain_threads, set_init_build_delay,
    BackendPreference, Reactor, ReactorConfig,
};

/// How many threads race to initialise the reactor.
const RACERS: usize = 16;

/// How long every builder is held inside the build body. This is what makes the
/// race deterministic: without it, the first thread can finish `REACTOR.set`
/// before the others even run their `REACTOR.get()` check, and a buggy build
/// would look correct by luck.
const BUILD_DELAY: Duration = Duration::from_secs(1);

/// Long enough that a leaked drain thread cannot exit during the sampling
/// window below, even though its `Drop` already set `shutdown`.
const LEAK_DRAIN_POLL_TIMEOUT: Duration = Duration::from_secs(10);

/// How long to sample the live-thread counter so a drain thread has time to
/// start and register itself.
const SAMPLE_WINDOW: Duration = Duration::from_secs(1);

#[test]
fn concurrent_initialisation_spawns_exactly_one_drain_thread() {
    assert_eq!(
        live_drain_threads(),
        0,
        "the reactor must not be initialised before this test starts"
    );

    set_init_build_delay(BUILD_DELAY);

    let barrier = Arc::new(Barrier::new(RACERS));
    let handles: Vec<_> = (0..RACERS)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                Reactor::init_with_config(
                    BackendPreference::Auto,
                    ReactorConfig::default().with_drain_poll_timeout(LEAK_DRAIN_POLL_TIMEOUT),
                )
                .expect("reactor initialises")
            })
        })
        .collect();

    let reactors: Vec<Arc<Reactor>> = handles
        .into_iter()
        .map(|handle| handle.join().expect("init thread does not panic"))
        .collect();

    // Every racer must receive the one winning instance.
    for pair in reactors.windows(2) {
        assert!(
            Arc::ptr_eq(&pair[0], &pair[1]),
            "every initialiser must return the same reactor instance"
        );
    }

    // Sample for a while: a loser's drain thread is spawned by its init thread
    // but may not be scheduled, and its counter increment, until after that
    // init thread has returned.
    let deadline = Instant::now() + SAMPLE_WINDOW;
    let mut max_live = live_drain_threads();
    while Instant::now() < deadline {
        max_live = max_live.max(live_drain_threads());
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(
        max_live, 1,
        "expected exactly one reactor-drain thread after the init race, found {max_live} \
         (each losing initialiser leaked one; issue #240)"
    );

    assert_eq!(
        init_build_attempts(),
        1,
        "exactly one initialiser may enter the reactor build body"
    );

    assert_eq!(
        drain_thread_spawn_count(),
        1,
        "exactly one reactor-drain thread may be spawned; one per racing loser \
         means initialisation is not single-flight (issue #240)"
    );
}
