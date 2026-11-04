//! A reconnect tells the consumer to start over (spec 70 feature 77, SSE stall).
//!
//! WHY: a server that ignores `Last-Event-ID` replays the response from its
//! first event. A consumer that accumulates text appended the replay to what it
//! already held and produced "Hello worldHello world".
//!
//! WHAT: the reconnecting task yields `Event::Reconnect` when the new connection
//! starts, before any replayed event, so a consumer can discard what it holds.

use foundation_core::valtron::{Ctx, TaskIterator, TaskStatus};
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
    while let Some(status) = task.next_status(Ctx::new()) {
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
    let calc = TimeoutCalculator::with_config(TimeoutConfig {
        sse_idle_timeout: Duration::from_secs(5),
        ..TimeoutConfig::default()
    });
    assert_eq!(calc.streaming_idle_timeout(), Duration::from_secs(5));
}
