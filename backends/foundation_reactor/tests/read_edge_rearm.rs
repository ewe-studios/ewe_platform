//! Registering a read waker must re-arm the fd's read edge.
//!
//! WHY: the macOS kqueue selector suppresses a readable event while the fd's
//! byte count does not grow (`poll/sys/unix/selector/kqueue.rs`, the
//! `read_edges` map). A socket that drains 77 bytes and then receives a
//! smaller 15-byte frame produces no new edge. Before this fix a task had to
//! call `FdReadiness::clear` before it registered its waker, or it never woke
//! (feature 129 lost the websocket echo this way). The reactor now re-arms the
//! edge inside the waker registration, so no caller can forget it.
//!
//! WHAT: three tests. The first sends a 77-byte head, drains it, registers a
//! waker, and then sends a 15-byte frame: the waker must fire. On the old code
//! the selector suppresses the smaller edge on macOS and the waker never
//! fires. The second test drains the frame and asserts the re-arm does not
//! re-wake forever. The third asserts `clear` still disarms the latched
//! `READABLE` bit.
//!
//! HOW: a loopback TCP pair drives the real process-wide reactor. The tests
//! wait on the reactor's event generation, never on a fixed sleep, so they
//! prove readiness rather than race it.

#![cfg(unix)]

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Wake, Waker};
use std::time::{Duration, Instant};

use foundation_reactor::FdReadiness;

/// The longest a correctness wait may take. Generous for a loaded CI runner,
/// short enough to fail a lost wake quickly.
const WAIT_BOUND: Duration = Duration::from_secs(2);

/// How long the spin guard watches for an unwanted extra wake.
const SPIN_GUARD: Duration = Duration::from_millis(200);

/// A waker that counts how many times it fires and lets a test park on the
/// count with a timeout. The mutex protects the count-versus-wait race: a wake
/// either increments before the wait starts or signals the parked waiter.
#[derive(Default)]
struct CountingWaker {
    wakes: AtomicUsize,
    lock: Mutex<()>,
    changed: Condvar,
}

impl CountingWaker {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn count(&self) -> usize {
        self.wakes.load(Ordering::Acquire)
    }

    /// Park until the waker fired at least `target` times, or return `false`
    /// when `timeout` passes first.
    fn wait_for_at_least(&self, target: usize, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if self.count() >= target {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let (next, _) = self
                .changed
                .wait_timeout(guard, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard = next;
        }
    }

    /// Park for `duration` and return the fire count seen at the end.
    fn count_after(&self, duration: Duration) -> usize {
        let deadline = Instant::now() + duration;
        let mut guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            let now = Instant::now();
            if now >= deadline {
                return self.count();
            }
            let (next, _) = self
                .changed
                .wait_timeout(guard, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard = next;
        }
    }
}

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.wakes.fetch_add(1, Ordering::AcqRel);
        self.changed.notify_all();
    }
}

/// A connected loopback TCP pair with a non-blocking client `reader`.
fn loopback_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback");
    let address = listener.local_addr().expect("loopback address");
    let reader = TcpStream::connect(address).expect("connect loopback");
    let (peer, _) = listener.accept().expect("accept loopback");
    reader.set_nonblocking(true).expect("non-blocking reader");
    (reader, peer)
}

/// Park until the reactor latches `READABLE` for the fd, or fail.
fn wait_for_readiness(readiness: &FdReadiness, label: &str) {
    let deadline = Instant::now() + WAIT_BOUND;
    while Instant::now() < deadline {
        // Snapshot the generation before the check: an event that lands in
        // the race window bumps it, so the wait below returns at once.
        let generation = readiness.events_generation();
        if readiness.ready().is_readable() {
            return;
        }
        // The return is the new generation; the loop re-snapshots it, so
        // discarding it here is deliberate.
        let _ = readiness.wait_for_events(generation, Duration::from_millis(100));
    }
    panic!("{label}: the fd did not report readability within {WAIT_BOUND:?}");
}

/// Read exactly `bytes.len()` bytes from a non-blocking stream, waiting for the
/// reactor-latched data to arrive. A `WouldBlock` before then is a short wait,
/// not a failure.
fn read_exactly(stream: &mut TcpStream, bytes: &mut [u8]) {
    let deadline = Instant::now() + WAIT_BOUND;
    let mut filled = 0;
    while filled < bytes.len() {
        match stream.read(&mut bytes[filled..]) {
            Ok(0) => panic!("the peer closed after {filled} of {} bytes", bytes.len()),
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "read timed out after {filled} of {} bytes",
                    bytes.len()
                );
                std::thread::yield_now();
            }
            Err(error) => panic!("read failed after {filled} bytes: {error}"),
        }
    }
}

/// One more read must report `WouldBlock` before the task parks.
fn assert_would_block(stream: &mut TcpStream) {
    let mut scratch = [0u8; 32];
    match stream.read(&mut scratch) {
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
        Ok(n) => panic!("expected WouldBlock, but the read returned {n} byte(s)"),
        Err(error) => panic!("expected WouldBlock, got {error}"),
    }
}

/// Drain everything buffered, then report that the stream is empty.
fn drain_available(stream: &mut TcpStream) {
    let mut chunks = [0u8; 64];
    loop {
        match stream.read(&mut chunks) {
            Ok(0) => return,
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
            Err(error) => panic!("drain failed: {error}"),
        }
    }
}

/// A 15-byte frame after a drained 77-byte head must wake the registered task.
///
/// This test fails on the old code on macOS: without the reactor's re-arm the
/// selector still records 77 bytes and suppresses the 15-byte event, so the
/// waker never fires.
#[test]
fn a_smaller_frame_after_a_drain_wakes_a_registered_task() {
    let (mut reader, mut peer) = loopback_pair();
    let readiness = FdReadiness::register(reader.as_raw_fd()).expect("register the reader");

    peer.write_all(&[b'H'; 77]).expect("write the head");
    wait_for_readiness(&readiness, "head");
    let mut head = [0u8; 77];
    read_exactly(&mut reader, &mut head);

    let counter = CountingWaker::new();
    let waker = Waker::from(Arc::clone(&counter));
    assert!(
        readiness.register_waker(&waker),
        "the reader must still be registered"
    );
    assert_would_block(&mut reader);

    peer.write_all(&[b'E'; 15]).expect("write the echo");
    assert!(
        counter.wait_for_at_least(1, WAIT_BOUND),
        "the 15-byte frame after a drained 77-byte head produced no wake: the \
         selector suppressed the smaller read edge"
    );
}

/// After the wake and a full drain, the re-armed edge must not wake again.
///
/// This guards the spin risk of re-arming on every registration.
#[test]
fn a_wake_after_a_drain_does_not_repeat_forever() {
    let (mut reader, mut peer) = loopback_pair();
    let readiness = FdReadiness::register(reader.as_raw_fd()).expect("register the reader");

    peer.write_all(&[b'H'; 77]).expect("write the head");
    wait_for_readiness(&readiness, "head");
    let mut head = [0u8; 77];
    read_exactly(&mut reader, &mut head);

    let counter = CountingWaker::new();
    let waker = Waker::from(Arc::clone(&counter));
    assert!(readiness.register_waker(&waker), "still registered");
    assert_would_block(&mut reader);

    peer.write_all(&[b'E'; 15]).expect("write the echo");
    assert!(
        counter.wait_for_at_least(1, WAIT_BOUND),
        "the 15-byte frame did not wake the task"
    );

    drain_available(&mut reader);
    readiness.clear();
    // Arm the next park. With no data buffered, this must not re-deliver the
    // edge that was already consumed.
    assert!(readiness.register_waker(&waker), "still registered");

    let settled = counter.count();
    let after = counter.count_after(SPIN_GUARD);
    assert_eq!(
        after, settled,
        "the re-armed edge woke the task again after a full drain"
    );
}

/// `clear` still disarms the latched `READABLE` bit.
///
/// The re-arm in registration covers the edge, but the latched flags are still
/// the caller's to clear; this guards that half of the protocol.
#[test]
fn clear_still_disarms_the_latched_readiness() {
    let (mut reader, mut peer) = loopback_pair();
    let readiness = FdReadiness::register(reader.as_raw_fd()).expect("register the reader");

    peer.write_all(&[b'H'; 16]).expect("write data");
    wait_for_readiness(&readiness, "data");
    let mut data = [0u8; 16];
    read_exactly(&mut reader, &mut data);
    assert!(
        readiness.ready().is_readable(),
        "the reactor must still latch READABLE until the caller clears it"
    );

    readiness.clear();
    assert!(
        !readiness.ready().is_readable(),
        "clear must disarm the latched READABLE bit"
    );
}
