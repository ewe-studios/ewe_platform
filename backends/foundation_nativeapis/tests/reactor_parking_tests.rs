//! End-to-end proof of native fd parking (spec-41 F10 / Decision 00 Level 2).
//!
//! WHY: Decision 00 decides that a native leaf task parks on the reactor by
//! returning `TaskStatus::Depends(Arc<dyn EventReadiness>)` and is re-run only
//! when the signal fires — **no new `ReadinessSource` trait, no `OnceLock`
//! slot** (superseded by `foundation_nativeapis`' reactor + `RegisteredFd:
//! EventReadiness`). These tests drive a real valtron task *through the
//! executor* to completion via `Depends`, proving the park-then-wake path with
//! (a) an in-tree test `EventReadiness` that a thread flips ready after a delay,
//! and (b) a real `RegisteredFd` whose pipe is written from another thread after
//! the task has already parked.
//!
//! Gated on `unix` + the `fd` feature (default) so the file is empty otherwise.
#![cfg(all(unix, feature = "fd"))]

use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use foundation_core::valtron::{
    collect_result, execute, initialize_pool, BoxedSendExecutionAction, EventReadiness,
    EventReadinessPtr, PoolGuard, TaskIterator, TaskStatus,
};
use foundation_nativeapis::native::fd::{Reactor, RegisteredFd};
use foundation_nativeapis::Token;

/// Initialize the valtron thread pool (multi executor is enabled for nativeapis).
fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

/// An in-tree `EventReadiness` backed by an atomic flag a separate thread flips.
struct FlipReadiness(Arc<AtomicBool>);

impl EventReadiness for FlipReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// A leaf task that parks on a readiness signal and completes once it fires.
///
/// First tick (signal not ready) → `Depends(signal)`, so the executor parks it.
/// When the signal becomes ready the executor re-runs the task → `Ready`. The
/// next tick terminates the iterator.
struct ParkUntilReady {
    /// `EventReadinessPtr` rather than a bare `Arc<dyn EventReadiness>`: under
    /// the `multi` feature `TaskStatus::Depends` carries a `Send + Sync` trait
    /// object so `State` can cross the global task queue.
    readiness: EventReadinessPtr,
    done: bool,
}

impl TaskIterator for ParkUntilReady {
    type Ready = &'static str;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.done {
            return None;
        }
        if self.readiness.is_ready(None) {
            self.done = true;
            Some(TaskStatus::Ready("woke"))
        } else {
            Some(TaskStatus::Depends(Arc::clone(&self.readiness)))
        }
    }
}

/// A leaf task that parks on a real `RegisteredFd` and, once woken, drains the
/// pending bytes and reports them as its terminal value.
///
/// It performs the real (nonblocking) read on each tick rather than re-checking
/// `is_ready()`: readiness only *wakes* the task (the executor's park gate polls
/// the edge-triggered fd and consumes the one-shot edge), so the woken task must
/// act on the buffered data directly. A `WouldBlock` read parks again via
/// `Depends(Arc<RegisteredFd>)`.
struct ReadOnReady {
    fd: Arc<RegisteredFd<std::os::fd::OwnedFd>>,
    done: bool,
}

impl TaskIterator for ReadOnReady {
    type Ready = Vec<u8>;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.done {
            return None;
        }
        let raw = self.fd.get_ref().as_raw_fd();
        let mut buf = [0u8; 64];
        let n = unsafe { libc::read(raw, buf.as_mut_ptr().cast(), buf.len()) };
        if n > 0 {
            self.done = true;
            Some(TaskStatus::Ready(buf[..n as usize].to_vec()))
        } else {
            // Nothing buffered yet (EAGAIN) — park until the reactor signals the
            // fd readable, then this task is re-run and the read above succeeds.
            Some(TaskStatus::Depends(
                Arc::clone(&self.fd) as EventReadinessPtr,
            ))
        }
    }
}

/// Create a nonblocking pipe (reader, writer).
fn make_pipe() -> (std::os::fd::OwnedFd, std::os::fd::OwnedFd) {
    let (r, w) = nix::unistd::pipe().expect("pipe creation failed");
    for fd in [r.as_raw_fd(), w.as_raw_fd()] {
        nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK))
            .expect("set O_NONBLOCK");
    }
    (r, w)
}

/// The generic parking path: a task parks on `Depends`, and a thread flipping an
/// `EventReadiness` ready after a delay unparks it to completion.
#[test]
#[foundation_macros::timeout(30_000)]
#[serial_test::serial]
fn readiness_signal_parks_task_until_flipped() {
    let _guard = init_pool();

    let flag = Arc::new(AtomicBool::new(false));
    let flag_bg = Arc::clone(&flag);
    thread::spawn(move || {
        // Task parks first; readiness becomes true only later.
        thread::sleep(Duration::from_millis(150));
        flag_bg.store(true, Ordering::Release);
    });

    let task = ParkUntilReady {
        readiness: Arc::new(FlipReadiness(flag)),
        done: false,
    };

    let stream = execute(task, Some(Duration::from_millis(20))).expect("execute");
    let results = collect_result(stream);

    assert_eq!(results, vec!["woke"], "parked task must complete once readiness fires");
}

/// Native fd parking end-to-end: the task parks on `Depends(Arc<RegisteredFd>)`
/// while the pipe is empty, then a thread writes after a delay and the reactor
/// unparks the task, which reads the bytes.
#[test]
#[foundation_macros::timeout(30_000)]
#[serial_test::serial]
fn registered_fd_parks_task_until_readable() {
    let _guard = init_pool();

    let (reader, writer) = make_pipe();
    // The shared reactor's registry, not a private `Poll` — this test exists to
    // prove the *shared* reactor wakes a parked task. Handing `RegisteredFd` a
    // caller-owned registry would register into a selector that has no drain
    // thread, and the task would park forever.
    let reactor = Reactor::get().expect("shared reactor");
    let registered = RegisteredFd::new(reader, reactor.registry(), Token(0)).expect("register fd");

    // Write only AFTER a delay, so the task must genuinely park first.
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        let msg = b"parked-then-woke";
        let n = unsafe { libc::write(writer.as_raw_fd(), msg.as_ptr().cast(), msg.len()) };
        assert!(n > 0, "pipe write should succeed");
        // Keep the writer open until after the write is observed.
        thread::sleep(Duration::from_millis(50));
        drop(writer);
    });

    let task = ReadOnReady {
        fd: Arc::new(registered),
        done: false,
    };

    let stream = execute(task, Some(Duration::from_millis(20))).expect("execute");
    let results = collect_result(stream);

    assert_eq!(results.len(), 1, "expected exactly one terminal read");
    assert_eq!(
        results[0], b"parked-then-woke",
        "task must read the bytes written after it parked"
    );
}
