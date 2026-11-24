//! A `send` wakes a WebSocket task that waits for a frame (GitHub issue #610).
//!
//! WHY: the Open state drained one queued message and then waited inside a
//! blocking read for the whole idle timeout. A `MessageDelivery::send` during
//! that wait was not delivered until the read timed out, so every interactive
//! client paid the read timeout per send.
//!
//! WHAT: parks the task on a quiet connection and proves two things. The task
//! parks at once, not inside a read. The parked signal reports ready as soon
//! as a message is queued.
//!
//! HOW: drives a `WebSocketTask` by hand against a local echo server, with an
//! idle timeout of 30 s, so any block in a read shows up as a failure.

#![cfg(feature = "multi")]

use std::time::{Duration, Instant};

use foundation_context::Ctx;
use foundation_core::valtron::{TaskIterator, TaskStatus};
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

#[cfg(unix)]
#[test]
#[foundation_macros::timeout(30000)]
fn a_send_wakes_a_task_waiting_for_a_frame() {
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    // The old latency: both waits are the read timeout of the blocking read.
    let old_latency = Duration::from_secs(30);
    let (delivery, rx) = MessageDelivery::new();
    let mut task = WebSocketTask::connect_with_delivery(
        SystemDnsResolver,
        url,
        None,
        SimpleHeaders::new(),
        rx,
        old_latency,
        old_latency,
    )
    .expect("task with a valid URL");

    let ctx = Ctx::host();
    establish(&mut task, &ctx);

    // The task parks on the outbound pipe and socket readiness.
    let started = Instant::now();
    let signal = match task.next_status(ctx.clone()) {
        Some(TaskStatus::Depends(signal)) => signal,
        other => panic!("a quiet connection must park, got {other:?}"),
    };
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "the task must park at once, not after the read timeout"
    );

    // A send reports the parked signal ready at once.
    delivery
        .send(WebSocketMessage::Text("wake".to_string()))
        .expect("should send");
    let sent = Instant::now();
    while !signal.is_ready(None) {
        assert!(
            sent.elapsed() < Duration::from_secs(1),
            "the send must wake the parked task at once"
        );
        std::thread::yield_now();
    }

    // The task sends the message and the echo arrives well under the timeout.
    let echo = Instant::now();
    loop {
        match task.next_status(ctx.clone()) {
            Some(TaskStatus::Ready(Ok(WebSocketMessage::Text(text)))) => {
                assert_eq!(text, "wake");
                break;
            }
            Some(
                TaskStatus::Pending(_)
                | TaskStatus::Depends(_)
                | TaskStatus::Delayed(_)
                | TaskStatus::Init,
            ) => {}
            other => panic!("unexpected status {other:?}"),
        }
        assert!(
            echo.elapsed() < Duration::from_secs(5),
            "the echo must arrive at once, not after the read timeout"
        );
    }
}
