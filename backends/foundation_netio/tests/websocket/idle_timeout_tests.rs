//! A quiet WebSocket waits the configured idle timeout, not the 100 ms
//! minimum read timeout (GitHub issue #288).
//!
//! WHY: the single-shot task used `TimeoutCalculator::calculate_read_timeout`
//! for its read wait. With the default calculator that value is
//! `min_read_timeout`, 100 ms, so a quiet connection closed almost at once.
//!
//! WHAT: drives a `WebSocketTask` by hand against a local echo server that
//! stays silent, then measures one read poll.
//!
//! HOW: the wait is a kernel socket read timeout (`SO_RCVTIMEO`), not a clock
//! comparison, so the test measures the real elapsed time. A simulated clock
//! cannot drive it.

#![cfg(feature = "multi")]

use std::time::{Duration, Instant};

use foundation_core::valtron::{Ctx, TaskIterator, TaskStatus};
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::SimpleHeaders;
use foundation_netio::websocket::{MessageDelivery, WebSocketMessage, WebSocketTask};
use foundation_testing::http::WebSocketEchoServer;

/// Drive the handshake until the connection is open.
fn establish(task: &mut WebSocketTask<SystemDnsResolver>, ctx: &Ctx) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match task.next_status(ctx.clone()) {
            Some(TaskStatus::Ready(Ok(WebSocketMessage::ConnectionEstablished))) => return,
            Some(TaskStatus::Delayed(delay)) => std::thread::sleep(delay),
            Some(_) => {}
            None => panic!("the connection closed during the handshake"),
        }
    }
    panic!("the handshake did not finish in 10 s");
}

#[test]
fn websocket_default_idle_timeout_is_the_nginx_keepalive_timeout() {
    let task = WebSocketTask::connect(SystemDnsResolver, "ws://127.0.0.1:1/idle")
        .expect("task with a valid URL");
    assert_eq!(
        task.idle_timeout(),
        Some(Duration::from_secs(75)),
        "the default is the nginx keepalive_timeout default"
    );

    let task = task.with_idle_timeout(Duration::from_millis(300));
    assert_eq!(task.idle_timeout(), Some(Duration::from_millis(300)));
}

/// A read that times out yields `Delayed` for the caller's `sleep_timeout` (the
/// idle poll interval), not for a value computed by the timeout calculator
/// (GitHub issue #375).
#[test]
#[foundation_macros::timeout(30000)]
fn websocket_delayed_uses_the_sleep_timeout() {
    let read_wait = Duration::from_millis(50);
    let sleep_between = Duration::from_millis(250);
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (_delivery, rx) = MessageDelivery::new();
    let mut task = WebSocketTask::connect_with_delivery(
        SystemDnsResolver,
        url,
        None,
        SimpleHeaders::new(),
        rx,
        read_wait,
        sleep_between,
    )
    .expect("task with a valid URL");

    let ctx = Ctx::host();
    establish(&mut task, &ctx);

    match task.next_status(ctx) {
        Some(TaskStatus::Delayed(delay)) => assert_eq!(
            delay, sleep_between,
            "the idle poll interval must be the caller's sleep timeout"
        ),
        other => panic!("a quiet connection must yield Delayed, got {other:?}"),
    }
}

#[test]
#[foundation_macros::timeout(30000)]
fn websocket_wait_uses_the_idle_timeout() {
    let configured = Duration::from_millis(300);
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (_delivery, rx) = MessageDelivery::new();
    let mut task = WebSocketTask::connect_with_delivery(
        SystemDnsResolver,
        url,
        None,
        SimpleHeaders::new(),
        rx,
        configured,
        Duration::from_millis(10),
    )
    .expect("task with a valid URL");

    let ctx = Ctx::host();
    establish(&mut task, &ctx);

    // The server stays silent, so this poll blocks on the read and then yields.
    let started = Instant::now();
    let status = task.next_status(ctx);
    let elapsed = started.elapsed();

    assert!(
        matches!(status, Some(TaskStatus::Delayed(_))),
        "a quiet connection must yield Delayed after its read wait"
    );
    assert!(
        elapsed >= Duration::from_millis(250),
        "the read wait lasted {elapsed:?}; it must use the configured 300 ms \
         idle timeout, not the 100 ms minimum read timeout"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "the read wait lasted {elapsed:?}; it did not stop at the configured idle timeout"
    );
}
