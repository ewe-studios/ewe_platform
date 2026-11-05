//! Spec 70 feature 82 — the browser broadcaster's SSE transport writes whole
//! frames through the async `SseStream`.
//!
//! `SseTransport` implements the synchronous `FrameTransport` trait, but
//! `SseStream::binary`/`message` are async. The transport pushes frames into a
//! bounded valtron `Pipe`; one driver task per client awaits the pipe and
//! writes each frame. This test drives the real `StreamHandler` over the
//! deterministic overlay transport and reads one byte per poll, so a truncated
//! frame fails the test.
#![cfg(unix)]

mod fake_overlay;

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_browser::test::stream::{SseTransportConfig, StreamHandler};
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory};
use foundation_netio::shared::http::SimpleIncomingRequest;
use foundation_wasm_ui::server::Broadcaster;

const FRAME: &[u8] = &[1, 2, 3, 4];

/// The wire event for [`FRAME`]: base64 of the four bytes on one `data:` line.
const EVENT: &str = "data: AQIDBA==\n\n";

/// Read a fake client one byte per poll until `EVENT` has arrived.
fn read_one_byte_per_poll(client: &fake_overlay::FakeClient) -> (Vec<u8>, bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got = Vec::new();
    while new_event_count(&got) == 0 && Instant::now() < deadline {
        let byte = client.read(1);
        if byte.is_empty() {
            std::thread::sleep(Duration::from_millis(1));
        } else {
            got.extend_from_slice(&byte);
        }
    }
    let complete = new_event_count(&got) > 0;
    (got, complete)
}

/// How many complete `EVENT`s `bytes` contains.
fn new_event_count(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes).matches(EVENT).count()
}

/// Wait until the handler has registered its connection with the broadcaster.
fn wait_for_connection(bc: &Broadcaster) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while bc.conn_count() == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    bc.conn_count() > 0
}

/// A slow browser receives the whole binary frame as one SSE event.
#[foundation_macros::valtron_test(seed = 823, threads = 3, timeout = 60_000)]
fn browser_sse_transport_delivers_the_whole_frame_to_a_slow_client() {
    let bag = Arc::new(ContextBag::new());
    let broadcaster = Broadcaster::new();
    bag.store(broadcaster.clone());
    bag.store(SseTransportConfig::new().with_queue_depth(16));

    let handler = StreamHandler::create(&bag);
    let (conn, client) = fake_overlay::overlay_pair(64);
    let request = SimpleIncomingRequest::builder()
        .with_plain_url("http://localhost/__primal/stream")
        .build()
        .expect("build the request");

    let response = handler.serve(Arc::clone(&bag), request, conn);
    let ConnectionResult::Async(future) = response else {
        panic!("the stream handler must upgrade asynchronously");
    };

    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let task = foundation_core::valtron::FutureTask::new(async move {
        let _ = done_tx.send(future.await);
    });
    foundation_core::valtron::send(task).expect("submit the stream handler task");

    let reader = std::thread::spawn(move || {
        let (got, complete) = read_one_byte_per_poll(&client);
        client.close();
        (got, complete)
    });

    assert!(
        wait_for_connection(&broadcaster),
        "the handler must register the SSE client"
    );
    broadcaster.sender().send(FRAME);

    let result = done_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the upgrade completes after registering the client");
    assert!(
        matches!(result, ConnectionResult::Take),
        "the handler must hand the connection to the frame driver"
    );

    let (got, complete) = reader.join().expect("reader thread");
    let text = String::from_utf8_lossy(&got);
    assert!(
        complete,
        "the client must receive the complete frame event, got {} bytes: {text:?}",
        got.len()
    );
    assert!(
        text.contains("HTTP/1.1 200"),
        "the SSE head must arrive: {text:?}"
    );
    assert_eq!(
        new_event_count(&got),
        1,
        "exactly one whole frame event: {text:?}"
    );
}
