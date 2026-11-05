//! A caught signal must not shorten a selector's wait.
//!
//! WHY: `epoll_wait` and `kevent` return `EINTR` when a handler catches a
//! signal. Treating that as "the timeout expired" makes every caller that
//! budgets a wait see a spurious expiry. The uring/epoll parity suite caught
//! it: on the CI runner `epoll` returned after 51 ms of a 150 ms wait, because
//! a signal interrupted the call and the selector reported an empty result.
//!
//! WHAT: install a `SIGUSR1` handler without `SA_RESTART`, send `SIGUSR1` to
//! the polling thread while it waits, and assert that the wait still lasts the
//! full timeout and still reports no readiness.
//!
//! HOW: this is its own test binary, because it installs a process-wide signal
//! handler and must not race other tests in the same process.

#![cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]

use std::io;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use foundation_reactor::{Events, Interest, Token};

#[cfg(any(target_os = "linux", target_os = "android"))]
use foundation_reactor::poll::sys::unix::selector::epoll::Selector;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
use foundation_reactor::poll::sys::unix::selector::kqueue::Selector;

/// The selector this platform dispatches through.
const BACKEND: &str = if cfg!(any(target_os = "linux", target_os = "android")) {
    "epoll"
} else {
    "kqueue"
};

extern "C" fn on_usr1(_signum: libc::c_int) {}

/// Install a `SIGUSR1` handler that catches the signal and returns.
///
/// `SA_RESTART` stays clear on purpose: with it the kernel resumes the
/// interrupted wait itself and the test would pass without ever exercising the
/// selector's retry. Returns the previous action so the caller can restore it.
fn catch_sigusr1() -> libc::sigaction {
    // SAFETY: a zeroed `sigaction` is the documented "no flags, empty mask"
    // starting point; `sigemptyset` and the field writes fully initialise it
    // before `sigaction(2)` reads it.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = on_usr1 as *const () as libc::sighandler_t;
        action.sa_flags = 0;
        libc::sigemptyset(&raw mut action.sa_mask);

        let mut previous: libc::sigaction = std::mem::zeroed();
        assert_eq!(
            libc::sigaction(libc::SIGUSR1, &raw const action, &raw mut previous),
            0,
            "installing the SIGUSR1 handler failed"
        );
        previous
    }
}

/// A pipe the test never reads from, so the fd stays idle and unready.
fn idle_pipe() -> (RawFd, RawFd) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` points at two `c_int`s, which is what `pipe(2)` writes.
    let r = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(r, 0, "pipe(2) failed: {}", io::Error::last_os_error());
    (fds[0], fds[1])
}

#[test]
fn a_signal_does_not_shorten_the_wait() {
    let previous = catch_sigusr1();

    let selector = Selector::new().expect("selector");
    let (read_fd, write_fd) = idle_pipe();
    selector
        .register_fd(read_fd, Token(1), Interest::READABLE)
        .expect("register the idle read end");

    // The signal must reach the thread that is parked in the wait, so send it
    // to this thread rather than to the process.
    // SAFETY: `pthread_self()` has no preconditions. `pthread_t` is `usize`
    // sized on every supported platform; passing it as `usize` keeps the helper
    // thread `Send`.
    let polling_thread = unsafe { libc::pthread_self() } as usize;

    let sender = std::thread::spawn(move || {
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(30));
            // SAFETY: the id belongs to a live thread (this test's thread) and
            // `SIGUSR1` has a handler installed for the whole test.
            unsafe { libc::pthread_kill(polling_thread as libc::pthread_t, libc::SIGUSR1) };
        }
    });

    let budget = Duration::from_millis(150);
    let mut events = Events::with_capacity(16);
    let start = Instant::now();
    let result = selector.poll(&mut events, Some(budget));
    let elapsed = start.elapsed();

    sender.join().expect("signal sender thread");

    // SAFETY: restores the action that this test replaced.
    unsafe { libc::sigaction(libc::SIGUSR1, &raw const previous, std::ptr::null_mut()) };
    selector.deregister_fd(read_fd).ok();
    // SAFETY: both ends are owned by this test and closed once.
    unsafe {
        libc::close(read_fd);
        libc::close(write_fd);
    }

    result.expect("a caught signal must not fail the wait");
    assert!(
        events.is_empty(),
        "{BACKEND}: the fd was never written, so the wait must report nothing, got {} event(s)",
        events.len()
    );
    assert!(
        elapsed >= Duration::from_millis(140),
        "{BACKEND}: poll returned after {elapsed:?} of a {budget:?} wait — a signal \
         truncated the timeout instead of resuming the wait"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "{BACKEND}: poll overran its {budget:?} wait, taking {elapsed:?}"
    );
}
