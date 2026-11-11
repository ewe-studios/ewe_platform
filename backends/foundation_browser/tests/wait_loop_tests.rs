//! The wait loops pause on the injected clock (GitHub issue 474).
//!
//! The loops in the launch path read their deadline from the `Ctx` clock. They
//! used to pause with `std::thread::sleep`. A frozen simulated clock never
//! moved, so the loops ran forever. Now the pause follows the same clock, and a
//! test steps it with `SimulatedClock::advance`.
//!
//! Each test runs the wait on a worker thread and reads the result through a
//! channel with a time limit. A hang shows as a test failure, not a stuck run.

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use foundation_browser::browser::connect_with_retry;
use foundation_browser::pause::{pause, WaitConfig};
use foundation_browser::supervisor::{discover_ws_url, wait_for_port};
use foundation_browser::Browser;
use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;

/// The real time a test waits for a result before it fails.
const BOUND: Duration = Duration::from_secs(10);
/// The real time a test waits to prove a frozen clock keeps a wait blocked.
const FROZEN_FOR: Duration = Duration::from_millis(150);

/// A wait config with a one-second clock poll and a quick real wake.
fn wait() -> WaitConfig {
    WaitConfig::default()
        .poll_interval(Duration::from_secs(1))
        .wake_interval(Duration::from_millis(1))
}

fn frozen() -> (Arc<SimulatedClock>, Ctx) {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock.clone());
    (clock, ctx)
}

/// A TCP port that nothing listens on.
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a port");
    listener.local_addr().expect("local addr").port()
}

/// Run `work` on a thread and return the receiver for its result.
fn spawn<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> mpsc::Receiver<T> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    rx
}

/// Assert the wait is still blocked while the clock is frozen, then step the
/// clock past the deadline and return the wait's result.
fn blocked_then_released<T>(
    rx: &mpsc::Receiver<T>,
    clock: &SimulatedClock,
    release: Duration,
) -> T {
    assert!(
        rx.recv_timeout(FROZEN_FOR).is_err(),
        "a frozen clock must keep the wait blocked"
    );
    clock.advance(release);
    rx.recv_timeout(BOUND)
        .expect("the wait must end after the clock passes the deadline")
}

#[test]
fn pause_blocks_until_the_clock_moves() {
    let (clock, ctx) = frozen();
    let rx = spawn(move || pause(&ctx, Duration::from_secs(3600), &wait()));
    blocked_then_released(&rx, &clock, Duration::from_secs(3600));
}

#[test]
fn wait_for_port_ends_when_the_clock_passes_the_deadline() {
    let (clock, ctx) = frozen();
    let port = closed_port();
    let rx = spawn(move || wait_for_port(port, Duration::from_secs(5), &wait(), &ctx));
    let result = blocked_then_released(&rx, &clock, Duration::from_secs(6));
    let err = result.expect_err("nothing listens on the port");
    assert!(err.to_string().contains("remote-debugging port"), "{err}");
}

#[test]
fn wait_for_port_succeeds_at_once_when_the_port_is_open() {
    let (_clock, ctx) = frozen();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    // The clock stays frozen: an open port must not need a single pause.
    let rx = spawn(move || wait_for_port(port, Duration::from_secs(5), &wait(), &ctx));
    rx.recv_timeout(BOUND)
        .expect("an open port must not wait")
        .expect("connect succeeds");
}

#[test]
fn discover_ws_url_ends_when_the_clock_passes_the_deadline() {
    let (clock, ctx) = frozen();
    let dir = std::env::temp_dir().join(format!("wait-loop-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("make the profile dir");
    let path = dir.clone();
    let rx = spawn(move || discover_ws_url(&path, Duration::from_secs(5), &wait(), &ctx));
    let result = blocked_then_released(&rx, &clock, Duration::from_secs(6));
    let _ = std::fs::remove_dir_all(&dir);
    let err = result.expect_err("no DevToolsActivePort file exists");
    assert!(err.to_string().contains("DevToolsActivePort"), "{err}");
}

#[test]
fn discover_ws_url_reads_a_ready_port_file_without_pausing() {
    let (_clock, ctx) = frozen();
    let dir = std::env::temp_dir().join(format!("wait-loop-ready-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("make the profile dir");
    std::fs::write(
        dir.join("DevToolsActivePort"),
        "9222\n/devtools/browser/abc\n",
    )
    .expect("write the port file");
    let path = dir.clone();
    let rx = spawn(move || discover_ws_url(&path, Duration::from_secs(5), &wait(), &ctx));
    let url = rx
        .recv_timeout(BOUND)
        .expect("a ready port file must not wait")
        .expect("the url is built");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(url, "ws://127.0.0.1:9222/devtools/browser/abc");
}

#[test]
fn connect_with_retry_ends_when_the_clock_passes_the_deadline() {
    let (clock, ctx) = frozen();
    // The socket connect is lazy and succeeds on a closed port, so a bad URL
    // is the way to make every attempt fail at once.
    let url = "not a websocket url".to_string();
    let rx = spawn(move || connect_with_retry(&url, Browser::Firefox, &wait(), &ctx).is_err());
    // The Firefox retry deadline is 20 s of clock time.
    let failed = blocked_then_released(&rx, &clock, Duration::from_secs(21));
    assert!(failed, "the connect must fail after the deadline");
}
