//! `FdRegistration` / `RegisteredFd` are event-driven on the reactor path
//! (spec 70 feature 77, E4).
//!
//! WHY: the shared reactor's drain thread latches readiness and then wakes the
//! waker of the task parked on the fd (`EventReadiness::register`), so an executor
//! need not re-check a parked fd on a 1 ms timer. The legacy private-`Poll` path has no such wake and keeps polling.
//!
//! WHAT: a task parks on a `RegisteredFd` over an empty pipe; another thread
//! writes 200 ms later. The task must read the bytes soon after, and a counting
//! adapter must show the executor checked the readiness only a few times.
#![cfg(all(unix, feature = "fd"))]

use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use foundation_context::Ctx;
use foundation_core::valtron::{
    collect_result, execute, initialize_pool, EventReadiness, EventReadinessPtr, PoolGuard,
    ReadinessWaker, TaskIterator, TaskStatus,
};
use foundation_nativeapis::native::fd::{Reactor, RegisteredFd};
use foundation_nativeapis::{Poll, Token};

/// Counts `is_ready` calls on the readiness it wraps.
struct Counting {
    inner: Arc<RegisteredFd<OwnedFd>>,
    checks: Arc<AtomicUsize>,
}

impl EventReadiness for Counting {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst);
        self.inner.is_ready(dur)
    }

    fn clear(&self) {
        self.inner.clear();
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.inner.register(waker)
    }
}

/// Reads the pipe; parks on the counted fd readiness on `WouldBlock`.
struct ReadOnReady {
    fd: Arc<RegisteredFd<OwnedFd>>,
    counted: EventReadinessPtr,
    done: bool,
}

impl TaskIterator for ReadOnReady {
    type Ready = Vec<u8>;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done {
            return None;
        }
        let mut buf = [0u8; 64];
        let n = unsafe { libc::read(self.fd.get_ref().as_raw_fd(), buf.as_mut_ptr().cast(), 64) };
        if n > 0 {
            self.done = true;
            Some(TaskStatus::Ready(buf[..n.cast_unsigned()].to_vec()))
        } else {
            Some(TaskStatus::Depends(Arc::clone(&self.counted)))
        }
    }
}

fn make_pipe() -> (OwnedFd, OwnedFd) {
    let (r, w) = nix::unistd::pipe().expect("pipe creation failed");
    for fd in [r.as_raw_fd(), w.as_raw_fd()] {
        nix::fcntl::fcntl(
            fd,
            nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
        )
        .expect("set O_NONBLOCK");
    }
    (r, w)
}

/// Park a task on `fd`, write to `writer` from another thread after 200 ms.
/// Returns (milliseconds until the bytes were read, readiness checks, bytes).
fn park_and_write(fd: RegisteredFd<OwnedFd>, writer: OwnedFd) -> (u128, usize, Vec<u8>) {
    let fd = Arc::new(fd);
    let checks = Arc::new(AtomicUsize::new(0));
    let counted: EventReadinessPtr = Arc::new(Counting {
        inner: Arc::clone(&fd),
        checks: Arc::clone(&checks),
    });
    let started = Instant::now();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let msg = b"from-another-thread";
        let n = unsafe { libc::write(writer.as_raw_fd(), msg.as_ptr().cast(), msg.len()) };
        assert!(n > 0, "pipe write");
        thread::sleep(Duration::from_millis(100));
        drop(writer);
    });
    let stream = execute(
        ReadOnReady {
            fd,
            counted,
            done: false,
        },
        None,
    )
    .expect("execute");
    let results = collect_result(stream);
    let ms = started.elapsed().as_millis();
    assert_eq!(results.len(), 1, "one terminal read");
    (ms, checks.load(Ordering::SeqCst), results[0].clone())
}

/// Reactor path: `register` takes the waker, the task wakes on the write, and the
/// executor checks the readiness only a few times while it is parked.
#[test]
#[foundation_macros::timeout(30_000)]
#[serial_test::serial]
fn reactor_registered_fd_wakes_on_write_without_polling() {
    let _guard: PoolGuard = initialize_pool(42, Some(3));
    let (reader, writer) = make_pipe();
    let reactor = Reactor::get().expect("shared reactor");
    let fd = RegisteredFd::new(reader, reactor.registry(), Token(0)).expect("register fd");
    assert!(
        fd.register(&ReadinessWaker::noop()),
        "reactor path is event-driven"
    );

    let (ms, checks, bytes) = park_and_write(fd, writer);
    eprintln!("reactor path: {ms} ms, {checks} checks");
    assert_eq!(bytes, b"from-another-thread");
    assert!((200..450).contains(&ms), "woke {ms} ms after start");
    assert!(
        checks <= 20,
        "{checks} checks while parked; the bound is 20"
    );
}

/// Legacy path (a caller-owned `Poll`): still polls, and still wakes.
#[test]
#[foundation_macros::timeout(30_000)]
#[serial_test::serial]
fn legacy_registered_fd_keeps_polling_and_wakes() {
    let _guard: PoolGuard = initialize_pool(42, Some(3));
    let (reader, writer) = make_pipe();
    let poll = Poll::new().expect("poll");
    let fd = RegisteredFd::new(reader, &poll.registry(), Token(1)).expect("register fd");
    assert!(
        !fd.register(&ReadinessWaker::noop()),
        "legacy path must keep polling"
    );

    let (ms, checks, bytes) = park_and_write(fd, writer);
    eprintln!("legacy path: {ms} ms, {checks} checks");
    assert_eq!(bytes, b"from-another-thread");
    assert!((200..450).contains(&ms), "woke {ms} ms after start");
    drop(poll);
}

/// `StopSignal::stop` resumes the executor, so a task parked on it is not
/// re-checked on a timer (spec 70 feature 77, E3).
#[test]
#[foundation_macros::timeout(30_000)]
#[serial_test::serial]
fn stop_signal_wakes_a_parked_task_without_polling() {
    use foundation_nativeapis::valtron::StopSignal;

    struct ParkUntilStopped {
        signal: EventReadinessPtr,
        started: Instant,
        step: u8,
    }

    impl TaskIterator for ParkUntilStopped {
        type Ready = u128;
        type Pending = ();

        fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
            match self.step {
                0 => {
                    self.step = 1;
                    Some(TaskStatus::Depends(Arc::clone(&self.signal)))
                }
                1 => {
                    self.step = 2;
                    Some(TaskStatus::Ready(self.started.elapsed().as_millis()))
                }
                _ => None,
            }
        }
    }

    struct CountingStop(StopSignal, Arc<AtomicUsize>);

    impl EventReadiness for CountingStop {
        fn is_ready(&self, dur: Option<Duration>) -> bool {
            self.1.fetch_add(1, Ordering::SeqCst);
            self.0.is_ready(dur)
        }

        fn register(&self, waker: &ReadinessWaker) -> bool {
            self.0.register(waker)
        }
    }

    let _guard: PoolGuard = initialize_pool(42, Some(3));
    let stop = StopSignal::new();
    assert!(stop.register(&ReadinessWaker::noop()));
    let checks = Arc::new(AtomicUsize::new(0));
    let stopper = stop.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        stopper.stop();
    });
    let task = ParkUntilStopped {
        signal: Arc::new(CountingStop(stop, Arc::clone(&checks))),
        started: Instant::now(),
        step: 0,
    };
    let results = collect_result(execute(task, None).expect("execute"));
    let checks = checks.load(Ordering::SeqCst);
    eprintln!("stop signal: {results:?} ms, {checks} checks");
    assert!(
        (200..450).contains(&results[0]),
        "woke at {} ms",
        results[0]
    );
    assert!(
        checks <= 20,
        "{checks} checks while parked; the bound is 20"
    );
}
