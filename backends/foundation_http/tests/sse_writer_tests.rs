//! Spec 70 feature 82 — an SSE event is written whole on a non-blocking socket.
//!
//! The old path (`EventWriter`) formats an event with `writeln!` on the
//! connection's `Write` stream. The socket is non-blocking, so a slow client
//! fills the send buffer and the write stops in the middle of the event:
//! `EventWriter::send` returns `WouldBlock` and the rest of the event is lost.
//! The new path (`SseWriter`) renders the event to bytes and writes them
//! through `ResponseWriter`, which parks on write readiness and resumes at the
//! byte offset the socket accepted. The client receives the whole event.
//!
//! Both halves drive the deterministic in-memory overlay transport in
//! `fake_overlay/mod.rs` (the one from `response_writer_timer_tests.rs`), so
//! the result is the same on macOS and Linux.
#![cfg(all(feature = "multi", unix))]

mod fake_overlay;

use std::time::Duration;

use fake_overlay::{overlay_pair, FakeClient};
use foundation_http::native::upgrade::SseWriter;
use foundation_http::native::writer::WriterConfig;
use foundation_netio::event_source::{EventWriter, SseEvent};

/// Drive a writer future on the valtron pool, as a server connection task does,
/// and wait for its result on this (blocking) test thread.
fn run_writer<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let task = foundation_core::valtron::FutureTask::new(async move {
        let _ = tx.send(future.await);
    });
    foundation_core::valtron::send(task).expect("submit the writer task");
    rx.recv_timeout(Duration::from_secs(30))
        .expect("the writer task finished")
}

/// Read `want` bytes on a test thread in `chunk`-sized pieces.
fn drain_on_thread(
    client: FakeClient,
    want: usize,
    chunk: usize,
    pace: Duration,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || client.drain(want, chunk, pace))
}

/// Characterises the bug the feature fixes: the synchronous `EventWriter` stops
/// in the middle of the event when the send buffer has no room.
///
/// Measured on this branch against the retained synchronous `EventWriter` path
/// (the feature keeps it for in-memory writers): the send buffer held 64 bytes
/// with 10 free; `EventWriter::send` of one 16 KiB event returned `WouldBlock`
/// after writing 10 bytes (`"data: "` plus the first 4 data bytes). The client
/// received 54 filler bytes and a 10-byte prefix of the event — no complete
/// event. On `c8c8e5080` the same event also stops after 10 bytes, but the
/// prefix starts with `event: relo` because the old writer emitted each field
/// with its own `writeln!`.
#[test]
fn synchronous_event_writer_truncates_an_event_on_a_full_socket() {
    let (conn, client) = overlay_pair(64);
    // Leave 10 bytes free, as a client that read a little and then stalled.
    client.fill_with_free(10);

    let line = "x".repeat(16 * 1024);
    let event = SseEvent::new().event("reload").data(line).build();
    let expected = format!("event: reload\ndata: {}\n\n", "x".repeat(16 * 1024)).into_bytes();
    let mut writer = EventWriter::new(conn);

    let result = writer.send(&event);
    let error = result.expect_err("the synchronous writer must fail on a full socket");
    assert_eq!(
        error.kind(),
        std::io::ErrorKind::WouldBlock,
        "the synchronous writer must fail with WouldBlock"
    );
    drop(writer);

    let got = client.read_available();
    assert_eq!(got.len(), 54 + 10, "54 filler bytes and a 10-byte prefix");
    assert!(
        got[54..].len() < expected.len(),
        "the old path must leave an incomplete event"
    );
    assert_eq!(
        &got[54..],
        &expected[..got.len() - 54],
        "the bytes are a prefix of the event, with no terminating blank line"
    );
    assert!(
        !got.ends_with(b"\n\n"),
        "a complete event ends with the dispatch blank line"
    );
}

/// The fix: `SseWriter` resumes a partial socket write instead of dropping the
/// rest of the event. The client reads at one byte per poll.
#[foundation_macros::valtron_test(seed = 821, threads = 3, timeout = 60_000)]
fn sse_writer_delivers_a_whole_event_to_a_slow_client() {
    let (conn, client) = overlay_pair(64);
    // A previous response left the send buffer full.
    client.fill();

    let event = SseEvent::new()
        .id("7")
        .event("reload")
        .data("x".repeat(4096))
        .build();
    let expected = event.rendered();
    let want = 64 + expected.len();
    // One byte per poll: the writer parks on every kernel-level write.
    let reader = drain_on_thread(client, want, 1, Duration::from_millis(1));

    let writer_event = event.clone();
    let result = run_writer(async move {
        let mut writer = SseWriter::new(
            conn,
            WriterConfig::default(),
            &foundation_core::valtron::Ctx::host(),
        );
        writer.send(&writer_event).await
    });
    assert!(
        result.is_ok(),
        "the async writer must deliver the event: {result:?}"
    );

    let got = reader.join().expect("reader thread");
    assert_eq!(
        got.len(),
        want,
        "the filler and the whole event must arrive"
    );
    assert_eq!(
        &got[64..],
        &expected[..],
        "the client must receive the complete event bytes"
    );
}
