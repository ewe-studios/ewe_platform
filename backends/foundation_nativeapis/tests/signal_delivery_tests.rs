//! Proof that a signal is delivered on the readiness poll, not in the handler.
//!
//! WHY: the Linux and Windows handlers run at an arbitrary point on an
//! arbitrary thread. A handler that locks the bus or allocates memory can
//! deadlock that thread or the allocator.
//! WHAT: the handler only records the signal. The poll of the platform
//! handle's readiness delivers it, and [`SignalTask`] parks on that handle.
//!
//! The tests send a real signal to this process, so the whole file is one
//! target: the signal handlers are process-global. Gated on `unix`, so the
//! Windows build compiles an empty target.

#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use foundation_compact::Instant;
use foundation_core::valtron::{Ctx, MonotonicClock, TaskIterator, TaskStatus};
use foundation_nativeapis::signal::{signal_task, SignalEvent, SignalKind};

/// A context with the real monotonic clock.
fn ctx() -> Ctx {
    Ctx::new(Arc::new(MonotonicClock::new()))
}

/// Install the signal disposition that the platform needs before the test
/// sends `SIGTERM`.
///
/// Linux and Windows have a handler: `register_signals` installs it, and it
/// records the signal. macOS has no handler, and `EVFILT_SIGNAL` does not
/// suppress a signal's default action (it only reports it), so an unhandled
/// `SIGTERM` would terminate the test process. Install a no-op handler here.
#[cfg(target_os = "macos")]
fn prepare_sigterm_handler() {
    extern "C" fn noop(_signum: libc::c_int) {}

    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = noop as *const () as libc::sighandler_t;
        libc::sigfillset(&raw mut action.sa_mask);
        action.sa_flags = 0;
        let result = libc::sigaction(libc::SIGTERM, &raw const action, std::ptr::null_mut());
        assert_eq!(result, 0, "sigaction(SIGTERM) must succeed");
    }
}

/// Linux and Windows install a handler during registration, so the test needs
/// no preparation.
#[cfg(not(target_os = "macos"))]
fn prepare_sigterm_handler() {}

/// Send `SIGTERM` to this process and prove that the kernel accepted it.
///
/// On Linux a thread-directed `raise` runs the crate's handler before it
/// returns, so the pending bit and the eventfd are set.
///
/// On macOS `raise` is thread-directed, and a thread-directed signal does not
/// reach a kqueue filter when a handler is installed. `kill` is
/// process-directed, and the kernel posts the kqueue event before it returns.
#[cfg(target_os = "macos")]
fn send_sigterm() {
    let result = unsafe { libc::kill(libc::getpid(), libc::SIGTERM) };
    assert_eq!(result, 0, "kill(SIGTERM) must succeed");
}

#[cfg(not(target_os = "macos"))]
fn send_sigterm() {
    let result = unsafe { libc::raise(libc::SIGTERM) };
    assert_eq!(result, 0, "raise(SIGTERM) must succeed");
}

#[test]
#[foundation_macros::timeout(30_000)]
#[serial_test::serial]
fn a_signal_waits_for_the_readiness_poll() {
    prepare_sigterm_handler();
    let (mut task, bus) = signal_task(Arc::new(MonotonicClock::new())).expect("register signals");
    let subscriber = bus.subscribe_events();

    send_sigterm();

    assert!(
        subscriber.is_empty(),
        "the handler must not deliver; only the readiness poll delivers"
    );

    let Some(TaskStatus::Depends(readiness)) = task.next_status(ctx()) else {
        panic!("the unpolled task must park with Depends");
    };
    assert!(
        readiness.is_ready(None),
        "the poll must report the signal that the handler recorded"
    );

    match task.next_status(ctx()) {
        Some(TaskStatus::Ready(SignalEvent { kind, .. })) => {
            assert_eq!(kind, SignalKind::Terminate);
        }
        _ => panic!("the task must return the delivered signal"),
    }
}

#[test]
#[foundation_macros::timeout(30_000)]
#[serial_test::serial]
fn a_nonblocking_poll_returns_at_once() {
    prepare_sigterm_handler();
    let (mut task, _bus) = signal_task(Arc::new(MonotonicClock::new())).expect("register signals");

    let Some(TaskStatus::Depends(readiness)) = task.next_status(ctx()) else {
        panic!("the unpolled task must park with Depends");
    };

    let started = Instant::now();
    let ready = readiness.is_ready(None);
    let elapsed = started.elapsed();

    assert!(!ready, "no signal is pending");
    assert!(
        elapsed < Duration::from_millis(250),
        "is_ready(None) must not block; it took {elapsed:?}"
    );
}

#[test]
#[foundation_macros::timeout(30_000)]
#[serial_test::serial]
fn a_signal_sent_before_the_park_is_not_lost() {
    prepare_sigterm_handler();
    let (mut task, _bus) = signal_task(Arc::new(MonotonicClock::new())).expect("register signals");

    // The signal arrives before the task ever parks. The platform edge (the
    // eventfd counter on Linux, the kqueue event on macOS) holds it.
    send_sigterm();

    let Some(TaskStatus::Depends(readiness)) = task.next_status(ctx()) else {
        panic!("the unpolled task must park with Depends");
    };
    assert!(
        readiness.is_ready(None),
        "the poll must find the signal that arrived before the park"
    );

    match task.next_status(ctx()) {
        Some(TaskStatus::Ready(SignalEvent { kind, .. })) => {
            assert_eq!(kind, SignalKind::Terminate);
        }
        _ => panic!("the task must return the signal that arrived before the park"),
    }
}
