//! A `Waker` ends a blocked poll, and a poll with nothing ready waits (#532).
//!
//! WHY: the Windows selector waited only on the registered sockets in
//! `WSAPoll`. A wake signalled an event handle that no poll watched, so it
//! did not end the wait. With no socket registered, the poll returned at once
//! and the drain loop spun. These tests run on every OS, so all selectors keep
//! the same contract.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use foundation_reactor::{Events, Poll, Token, Waker};

const WAKER: Token = Token(9);

#[test]
fn a_wake_ends_a_blocked_poll_and_reports_the_waker_token() {
    let poll = Poll::new().expect("poll");
    let waker = Arc::new(Waker::new(&poll.registry(), WAKER).expect("waker"));

    let remote = Arc::clone(&waker);
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        remote.wake().expect("wake");
    });

    let mut events = Events::with_capacity(8);
    let start = Instant::now();
    let deadline = start + Duration::from_secs(10);
    let mut woken = false;
    while !woken && Instant::now() < deadline {
        events.clear();
        poll.poll(&mut events, Some(Duration::from_secs(10)))
            .expect("poll");
        woken = events.iter().any(|e| e.token() == WAKER);
    }
    handle.join().expect("waker thread");

    assert!(woken, "the poll never reported the waker token");
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "the wake did not end the blocked poll: it took {:?}",
        start.elapsed()
    );
}

#[test]
fn a_poll_with_nothing_ready_waits_for_its_timeout() {
    let poll = Poll::new().expect("poll");
    let mut events = Events::with_capacity(8);

    let start = Instant::now();
    poll.poll(&mut events, Some(Duration::from_millis(200)))
        .expect("poll");
    let waited = start.elapsed();

    assert!(
        events.iter().next().is_some() || waited >= Duration::from_millis(150),
        "a poll with nothing registered returned after {waited:?} with no event: it spins"
    );
}
