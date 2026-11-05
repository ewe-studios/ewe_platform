//! `HttpClient::send_sse` and `send_sse_async` deliver a stream as the server sends
//! it (spec 70 feature 80, SSE stall).
//!
//! WHY: on PR 173 the `foundation_ai` provider test `test_provider_streaming_text`
//! produced "Hello worldHello world". The mock server answers with a `Content-Length`
//! and keeps the connection open. The client waited past the last byte for end of
//! file, the read timeout ended the stream, the task reconnected, and the replayed
//! response was appended to the first one.
//!
//! WHAT: these tests use the same path as the providers (`NativeHttpClient` ->
//! `ReconnectingEventSourceTask`) with the same kind of server. Each stream runs on
//! its own thread with a deadline, so a stall fails the test instead of hanging it.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use foundation_core::valtron::Stream;
use foundation_netio::event_source::{Event, ParseResult};
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::client::request_builder::PreparedRequestBuilder;
use foundation_netio::shared::http::SimpleMethod;
use foundation_netio::HttpClientBuilder;
use foundation_testing::http::{
    HttpResponse, SseConnectionResult, SseStreamWriter, SseTestServer, TestHttpServer,
};
use serial_test::serial;

fn addr_of(base_url: &str) -> SocketAddr {
    base_url
        .strip_prefix("http://")
        .expect("http url")
        .parse()
        .expect("socket address")
}

fn request(base_url: &str) -> PreparedRequest {
    PreparedRequestBuilder::new(SimpleMethod::POST, base_url)
        .expect("url")
        .header(
            foundation_netio::shared::http::SimpleHeader::ACCEPT,
            "text/event-stream".to_string(),
        )
        .body_text("{}")
        .build()
}

/// What a consumer sees: the data of each message, `reconnect` for a restart marker.
fn label(parse_result: &ParseResult) -> Option<String> {
    match &parse_result.event {
        Event::Message { data, .. } => Some(data.clone()),
        Event::Reconnect => Some("reconnect".to_string()),
        Event::Comment(_) => None,
    }
}

/// A server that answers with a `Content-Length` and keeps the connection open (the
/// shape of the provider mock servers).
#[test]
#[serial(valtron_pool)]
fn send_sse_delivers_a_content_length_stream_once_and_ends() {
    let _pool_guard = foundation_core::valtron::initialize_pool(42, None);
    let body: &'static [u8] =
        b"event: delta\ndata: Hello\n\nevent: delta\ndata:  world\n\nevent: stop\ndata: done\n\n";
    let server = TestHttpServer::with_response(move |_req| HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "text/event-stream".to_string()),
            ("Cache-Control".to_string(), "no-cache".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
        ],
        body: body.to_vec(),
    });
    let url = server.base_url().to_string();
    let addr = addr_of(&url);

    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let client = HttpClientBuilder::new().resolve_to(addr).build();
        let started = Instant::now();
        let stream = client.send_sse(request(&url)).expect("send_sse");
        let seen: Vec<String> = stream
            .filter_map(|item| match item {
                Stream::Next(parse_result) => label(&parse_result),
                _ => None,
            })
            .collect();
        let _ = done.send((seen, started.elapsed()));
    });
    let (seen, elapsed) = finished
        .recv_timeout(Duration::from_secs(20))
        .expect("the stream stalled after its last event");
    assert_eq!(seen, vec!["Hello", " world", "done"], "each event once");
    assert!(
        elapsed < Duration::from_secs(2),
        "the stream must end at its Content-Length, not at the read timeout: {elapsed:?}"
    );
}

/// An async consumer is woken by each event of a slow stream: the events come 400 ms
/// apart and each reaches the consumer about then, with no gap of the 3 s read timeout.
#[test]
#[serial(valtron_pool)]
fn send_sse_async_consumer_is_woken_by_each_slow_event() {
    let _pool_guard = foundation_core::valtron::initialize_pool(42, None);
    let server = SseTestServer::with_handler(
        move |_req: &foundation_testing::http::HttpRequest,
              writer: &mut SseStreamWriter,
              _conn: usize| {
            for word in ["one", "two", "three"] {
                let _ = writer.event(word);
                std::thread::sleep(Duration::from_millis(400));
            }
            SseConnectionResult::clean_close()
        },
    );
    let url = server.base_url().to_string();
    let addr = addr_of(&url);

    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let client = HttpClientBuilder::new().resolve_to(addr).build();
        let started = Instant::now();
        let arrivals = smol::block_on(async {
            let mut stream = client
                .send_sse_async(request(&url))
                .await
                .expect("send_sse_async");
            let mut arrivals: Vec<(String, Duration)> = Vec::new();
            while let Some(item) = futures_lite::StreamExt::next(&mut stream).await {
                if let Stream::Next(parse_result) = item {
                    if let Some(data) = label(&parse_result) {
                        arrivals.push((data, started.elapsed()));
                    }
                }
            }
            arrivals
        });
        let _ = done.send(arrivals);
    });
    let arrivals = finished
        .recv_timeout(Duration::from_secs(30))
        .expect("the async stream stalled");
    let words: Vec<&str> = arrivals.iter().map(|(word, _)| word.as_str()).collect();
    assert_eq!(
        words,
        vec!["one", "two", "three"],
        "each event once, in order"
    );
    for pair in arrivals.windows(2) {
        let gap = pair[1].1.saturating_sub(pair[0].1);
        assert!(
            gap < Duration::from_millis(1500),
            "events {} and {} were {gap:?} apart; the server sends one every 400 ms",
            pair[0].0,
            pair[1].0
        );
    }
}
