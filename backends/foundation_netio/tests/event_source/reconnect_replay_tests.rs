//! A reconnect tells the consumer to start over (spec 70 feature 77, SSE stall).
//!
//! WHY: a server that ignores `Last-Event-ID` replays the response from its
//! first event. A consumer that accumulates text appended the replay to what it
//! already held and produced "Hello worldHello world".
//!
//! WHAT: the reconnecting task yields `Event::Reconnect` when the new connection
//! starts, before any replayed event, so a consumer can discard what it holds.

use foundation_core::valtron::{TaskIterator, TaskStatus};

use foundation_context::Ctx;
use foundation_netio::event_source::native::ReconnectingEventSourceTask;
use foundation_netio::event_source::Event;
use foundation_netio::shared::client::StaticSocketAddr;
use foundation_netio::shared::http::timeout::{TimeoutCalculator, TimeoutConfig};
use foundation_testing::http::{SseConnectionResult, SseStreamWriter, SseTestServer};
use serial_test::serial;
use std::net::SocketAddr;
use std::time::Duration;

/// The first connection sends "Hello" and drops; the second replays "Hello" and
/// sends " world". Returns what a consumer holds at the end, once with the reset
/// on `Event::Reconnect` and the number of markers seen.
fn run_with_one_reconnect() -> (String, usize, Vec<String>) {
    let _pool_guard = foundation_core::valtron::initialize_pool(42, None);
    let server = SseTestServer::with_handler(
        move |_req: &foundation_testing::http::HttpRequest,
              writer: &mut SseStreamWriter,
              conn_num: usize| {
            let _ = writer.event("Hello");
            if conn_num == 0 {
                // The abrupt drop resets the connection (`SO_LINGER = 0`), and a
                // reset discards data that is still in the send buffer. Give the
                // client time to read the event first, so the test always starts
                // with the first "Hello". The client stays idle during this wait,
                // well below the stream idle timeout.
                std::thread::sleep(Duration::from_millis(100));
                SseConnectionResult::abrupt_drop()
            } else {
                let _ = writer.event(" world");
                SseConnectionResult::clean_close()
            }
        },
    );
    let addr: SocketAddr = server
        .base_url()
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .unwrap();
    let mut task =
        ReconnectingEventSourceTask::connect(StaticSocketAddr::new(addr), server.base_url())
            .unwrap()
            .with_max_retries(3);

    let mut text = String::new();
    let mut markers = 0;
    let mut order = Vec::new();
    let mut steps = 0;
    while let Some(status) = task.next_status(Ctx::host()) {
        if let TaskStatus::Ready(parse_result) = status {
            match parse_result.event {
                Event::Reconnect => {
                    markers += 1;
                    text.clear();
                    order.push("reconnect".to_string());
                }
                Event::Message { data, .. } => {
                    text.push_str(&data);
                    order.push(data);
                }
                Event::Comment(_) => {}
            }
        }
        steps += 1;
        assert!(steps < 500, "too many steps, possible infinite loop");
    }
    (text, markers, order)
}

#[test]
#[serial(valtron_pool)]
fn a_reconnect_marks_the_restart_so_replayed_text_is_not_doubled() {
    let (text, markers, order) = run_with_one_reconnect();
    assert_eq!(markers, 1, "one reconnect, one marker: {order:?}");
    assert_eq!(
        order,
        vec!["Hello", "reconnect", "Hello", " world"],
        "the marker comes before the replayed events"
    );
    assert_eq!(text, "Hello world", "the replay must not be appended");
}

/// The idle timeout of an open stream is configuration with a sane default, not
/// the size-based read timeout (100 ms for an unsized body), which reconnected a
/// stream that was only quiet for a moment.
#[test]
fn stream_idle_timeout_defaults_to_sixty_seconds_and_is_configurable() {
    assert_eq!(
        TimeoutCalculator::default().streaming_idle_timeout(),
        Duration::from_secs(60)
    );
    let calc = TimeoutCalculator::with_config(
        TimeoutConfig::default().with_sse_idle_timeout(Duration::from_secs(5)),
    );
    assert_eq!(calc.streaming_idle_timeout(), Duration::from_secs(5));
}

/// A complete SSE response that has a `Content-Length` ends at that length, even
/// when the server keeps the connection open (the provider tests do this).
///
/// WHY: the reader waited for end of file past the last byte. With the test
/// server the wait did not end at all: the stream stalled after its last event
/// (in the provider test the stall ended on the read timeout, the task reported a
/// stream error, reconnected, and the replayed response was appended to the first
/// one: "Hello worldHello world").
///
/// The stream runs on its own thread so a stall fails the test instead of hanging it.
#[test]
#[serial(valtron_pool)]
fn a_response_with_content_length_ends_without_a_reconnect() {
    use foundation_testing::http::{HttpResponse, TestHttpServer};

    let _pool_guard = foundation_core::valtron::initialize_pool(42, None);
    let body: &'static [u8] = b"event: a\ndata: Hello\n\nevent: a\ndata:  world\n\n";
    let server = TestHttpServer::with_response(move |_req| HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "text/event-stream".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
        ],
        body: body.to_vec(),
    });
    let addr: SocketAddr = server
        .base_url()
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .unwrap();
    let url = server.base_url().to_string();

    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut task = ReconnectingEventSourceTask::connect(StaticSocketAddr::new(addr), &url)
            .unwrap()
            .with_max_retries(3);
        let started = std::time::Instant::now();
        let mut order = Vec::new();
        let mut steps = 0;
        while let Some(status) = task.next_status(Ctx::host()) {
            if let TaskStatus::Ready(parse_result) = status {
                match parse_result.event {
                    Event::Reconnect => order.push("reconnect".to_string()),
                    Event::Message { data, .. } => order.push(data),
                    Event::Comment(_) => {}
                }
            }
            steps += 1;
            if steps >= 500 {
                break;
            }
        }
        let _ = done.send((order, started.elapsed()));
    });
    let (order, elapsed) = finished
        .recv_timeout(Duration::from_secs(20))
        .expect("the stream stalled after its last event instead of ending at its Content-Length");
    assert_eq!(
        order,
        vec!["Hello", " world"],
        "each event once, no reconnect"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "the stream must end at its Content-Length, not at the read timeout: {elapsed:?}"
    );
}
