//! Spec 70 feature 82 — the proxy SSE reload stream writes whole events.
//!
//! `SseReloadHandler` used to write the response head and every event with a
//! synchronous `EventWriter` on the non-blocking connection. A client that reads
//! slowly filled the socket, the write stopped in the middle of an event, and
//! the rest of the event was lost. The handler now runs as the connection's
//! async continuation and writes through `SseWriter`/`ResponseWriter`, which
//! parks on write readiness and resumes the event.
//!
//! The test drives the real handler over the deterministic in-memory overlay
//! transport (`fake_overlay/mod.rs`) and reads one byte per poll.
#![cfg(unix)]

mod fake_overlay;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{self, FutureTask, MonotonicClock};
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve::{ConnectionResult, ServeOutcome};
use foundation_netio::shared::http::SimpleIncomingRequest;
use foundation_toolings::proxy::SseReloadHandler;
use foundation_toolings::watcher::FileChange;

/// One complete `reload` event, exactly as the W3C format and the runtime
/// expect it.
const EVENT: &[u8] = b"event: reload\ndata: ready\n\n";

/// Read a fake client one byte per poll until `EVENT` has arrived, then close
/// the connection and queue one more event so the handler observes the failure
/// and returns.
fn read_one_byte_per_poll(
    client: &fake_overlay::FakeClient,
    queue: &ConcurrentQueue<FileChange>,
) -> (Vec<u8>, bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got = Vec::new();
    while !got.ends_with(EVENT) && Instant::now() < deadline {
        let byte = client.read(1);
        if byte.is_empty() {
            std::thread::sleep(Duration::from_millis(1));
        } else {
            got.extend_from_slice(&byte);
        }
    }
    let complete = got.ends_with(EVENT);
    client.close();
    queue
        .push(FileChange::Rust(PathBuf::from("src/lib.rs")))
        .expect("queue the event that wakes the closed connection");
    (got, complete)
}

/// A client that reads one byte per poll receives the whole event.
#[foundation_macros::valtron_test(seed = 822, threads = 3, timeout = 60_000)]
fn proxy_sse_client_reading_one_byte_per_poll_receives_the_whole_event() {
    let ctx = Ctx::new(Arc::new(MonotonicClock::new()));
    let queue = Arc::new(ConcurrentQueue::unbounded());
    ctx.store::<Arc<ConcurrentQueue<FileChange>>>(Arc::clone(&queue));
    let handler = SseReloadHandler::create(&ctx);

    // A reload change is already waiting when the handler starts.
    queue
        .push(FileChange::Rust(PathBuf::from("src/main.rs")))
        .expect("queue the reload");

    let (conn, client) = fake_overlay::overlay_pair(64);
    let request = SimpleIncomingRequest::builder()
        .with_plain_url("http://localhost/__reload")
        .build()
        .expect("build the request");

    let response = handler.serve(ctx.clone(), request, conn);
    let ConnectionResult::Async(future) = response else {
        panic!("the SSE reload handler must continue asynchronously");
    };

    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let task = FutureTask::new(async move {
        let _ = done_tx.send(future.await);
    });
    valtron::send(task).expect("submit the SSE handler task");

    let reader = std::thread::spawn(move || read_one_byte_per_poll(&client, &queue));
    let (got, complete) = reader.join().expect("reader thread");

    let text = String::from_utf8_lossy(&got);
    assert!(
        complete,
        "the client must receive the complete event, got {} bytes: {text:?}",
        got.len()
    );
    assert!(text.contains("HTTP/1.1 200"), "response head: {text:?}");
    assert!(
        text.to_ascii_lowercase()
            .contains("content-type: text/event-stream"),
        "SSE content type: {text:?}"
    );
    assert_eq!(
        text.matches("event: reload\ndata: ready\n\n").count(),
        1,
        "exactly one whole event: {text:?}"
    );

    // Closing the connection makes the next write fail, so the handler returns
    // instead of looping forever.
    let result = done_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the handler stops after the connection closes");
    assert!(
        matches!(result, ServeOutcome::Take | ServeOutcome::Close(_)),
        "a failed write must end the handler"
    );
}
