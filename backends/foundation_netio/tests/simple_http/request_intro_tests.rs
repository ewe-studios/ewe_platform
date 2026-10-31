//! `GetRequestIntroTask` silent-peer regressions (moved from `src/http/tasks/request_intro.rs`).

use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::ReadTimeoutOperations;
use foundation_core::url::Uri;
use foundation_core::valtron::{Ctx, EventReadiness, TaskIterator, TaskStatus};
use foundation_netio::http::{GetRequestIntroTask, HttpClientConnection, RequestIntro};
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::{HttpClientError, HttpReaderError};

/// Regression: a blocking transport whose read times out must yield exactly
/// one `Ready(Failed(WouldBlock))` and then end (`None`).
///
/// WHY: `park_on_would_block` used to store the resume state *before*
/// checking whether the transport could park. On a blocking `TcpStream` it
/// returned `Ready(Failed)` but left `IntroReading` in place, so the executor
/// drove the task again: another blocking read, another timeout, another
/// `Failed` — forever (CI `foundation_connectrpc` hung for 60 minutes logging
/// "[INTRO] read would block on a blocking transport" every 3s).
///
/// HOW: a loopback peer accepts and never writes; the client read timeout is
/// 100ms. The task is driven with a hard iteration cap and a wall-clock bound,
/// so a regression fails fast instead of hanging.
#[test]
fn blocking_transport_timeout_fails_once_and_terminates() {
    const MAX_STEPS: usize = 16;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");

    // Silent peer: accept, hold the socket open (never respond) until told to stop.
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let peer = std::thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept client");
        // An explicit stop, the sender dropping, or the cap all end the wait;
        // which one it was does not matter to the peer.
        let _stopped = stop_rx.recv_timeout(Duration::from_secs(30));
        drop(socket);
    });

    let uri = Uri::parse(&format!("http://127.0.0.1:{}/", addr.port())).expect("uri");
    let mut conn = HttpClientConnection::connect(
        &uri,
        &SystemDnsResolver::new(),
        Some(Duration::from_secs(5)),
    )
    .expect("connect to silent peer");
    conn.stream_mut()
        .set_read_timeout_as(Duration::from_millis(100))
        .expect("set read timeout");

    let mut task = GetRequestIntroTask::new(conn);

    let started = Instant::now();
    let mut ready_failures = 0usize;
    let mut terminated = false;
    for _ in 0..MAX_STEPS {
        match task.next_status(Ctx::new()) {
            None => {
                terminated = true;
                break;
            }
            Some(TaskStatus::Ready(RequestIntro::Failed(HttpClientError::ReaderError(
                HttpReaderError::WouldBlock,
            )))) => ready_failures += 1,
            Some(TaskStatus::Ready(RequestIntro::Failed(other))) => {
                panic!("unexpected failure kind: {other:?}")
            }
            Some(TaskStatus::Ready(RequestIntro::Success { .. })) => {
                panic!("a silent peer cannot produce a response")
            }
            Some(TaskStatus::Depends(_)) => {
                panic!("a blocking TcpStream must not park on a read waker")
            }
            Some(_) => {}
        }
    }
    let elapsed = started.elapsed();

    stop_tx.send(()).expect("signal silent peer");
    peer.join().expect("silent peer thread");

    assert!(
        terminated,
        "task kept running after its terminal failure (unbounded retry loop)"
    );
    assert_eq!(ready_failures, 1, "exactly one terminal failure is reported");
    assert!(
        elapsed < Duration::from_secs(10),
        "task took {elapsed:?}; expected a single ~100ms read timeout"
    );
}

/// Non-blocking counterpart of the #76 regression test: a silent peer on a
/// non-blocking socket must park (`Depends`) without holding a worker. Once
/// the head deadline passes, the task reports exactly one
/// `Failed(Timeout)` and ends.
///
/// WHY: the blocking test above pins "a timed-out blocking read fails once".
/// With non-blocking sockets nothing in the kernel times out any more, so the
/// deadline must do it, and must also terminate the task (no retry loop).
///
/// HOW: drive the task by hand. Expect a `Depends` whose signal is not ready
/// at first (the peer is silent) and turns ready by itself at the deadline.
/// One more step must then fail with `Timeout`, and the next must end.
#[test]
fn nonblocking_silent_peer_parks_then_times_out_once() {
    const MAX_STEPS: usize = 16;
    let head_timeout = Duration::from_millis(200);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let peer = std::thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept client");
        let _stopped = stop_rx.recv_timeout(Duration::from_secs(30));
        drop(socket);
    });

    let uri = Uri::parse(&format!("http://127.0.0.1:{}/", addr.port())).expect("uri");
    let conn = HttpClientConnection::connect(
        &uri,
        &SystemDnsResolver::new(),
        Some(Duration::from_secs(5)),
    )
    .expect("connect to silent peer");
    conn.set_nonblocking(true).expect("non-blocking");

    let mut task = GetRequestIntroTask::new(conn).with_head_timeout(head_timeout);

    let started = Instant::now();
    let mut parks = 0usize;
    let mut timeouts = 0usize;
    let mut terminated = false;
    for _ in 0..MAX_STEPS {
        match task.next_status(Ctx::new()) {
            None => {
                terminated = true;
                break;
            }
            Some(TaskStatus::Depends(signal)) => {
                parks += 1;
                assert!(
                    !signal.is_ready(None),
                    "a silent peer must not report readiness when parking"
                );
                // The executor would hold the task here. Emulate it: wait for
                // the signal to turn ready, which the deadline guarantees.
                let wait = Instant::now();
                while !signal.is_ready(None) {
                    assert!(
                        wait.elapsed() < Duration::from_secs(5),
                        "the park signal never fired; the deadline is missing"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            Some(TaskStatus::Ready(RequestIntro::Failed(HttpClientError::Timeout))) => {
                timeouts += 1;
            }
            Some(TaskStatus::Ready(RequestIntro::Failed(other))) => {
                panic!("unexpected failure kind: {other:?}")
            }
            Some(TaskStatus::Ready(RequestIntro::Success { .. })) => {
                panic!("a silent peer cannot produce a response")
            }
            Some(_) => {}
        }
    }
    let elapsed = started.elapsed();

    stop_tx.send(()).expect("signal silent peer");
    peer.join().expect("silent peer thread");

    assert!(parks >= 1, "the task must park on readiness, not fail at once");
    assert_eq!(timeouts, 1, "exactly one terminal timeout is reported");
    assert!(terminated, "task kept running after its terminal failure");
    assert!(
        elapsed >= head_timeout && elapsed < Duration::from_secs(5),
        "task took {elapsed:?}; expected about the {head_timeout:?} head deadline"
    );
}
