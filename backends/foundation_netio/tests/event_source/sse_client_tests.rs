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

use foundation_core::valtron::{valtron_test, Stream};
use foundation_netio::event_source::{Event, ParseResult};
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::client::request_builder::PreparedRequestBuilder;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleMethod};
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
//
// NOTE: this test predates the `#[valtron_test]` macro and keeps the old
// `#[serial(valtron_pool)]` + `initialize_pool` pattern. That is an
// anti-pattern: new tests must use `#[valtron_test]`. Do not copy this.
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
/// An async consumer is woken by each event of a slow stream: the events come 400 ms
/// apart and each reaches the consumer about then, with no gap of the 3 s read timeout.
//
// NOTE: this test predates the `#[valtron_test]` macro and keeps the old
// `#[serial(valtron_pool)]` + `initialize_pool` + `smol::block_on` pattern.
// That is an anti-pattern: new tests must use `#[valtron_test]` (sync or async).
// Do not copy this.
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

/// A chunked Server-Sent Events (SSE) response must be de-chunked before the
/// event parser runs. The reference server sends `Transfer-Encoding: chunked`.
/// Without de-chunking, the parser reads the chunk-size lines as event fields
/// and loses the `event:` type.
#[valtron_test(timeout = 10_000)]
async fn send_async_parses_a_chunked_sse_body() {
    let data_event = b"event: data\ndata:hello\n\n";
    let control_event = b"event: control\ndata:{\"streamNextOffset\":\"00ff\",\"upToDate\":true}\n\n";
    let mut body = Vec::new();
    for event in [data_event.as_slice(), control_event.as_slice()] {
        body.extend_from_slice(format!("{:x}\r\n", event.len()).as_bytes());
        body.extend_from_slice(event);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"0\r\n\r\n");

    let server = TestHttpServer::with_response(move |_req| HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "text/event-stream".to_string()),
            ("Transfer-Encoding".to_string(), "chunked".to_string()),
        ],
        body: body.clone(),
    });
    let url = server.base_url().to_string();
    let addr = addr_of(&url);

    let client = HttpClientBuilder::new().resolve_to(addr).build();
    let req = PreparedRequestBuilder::new(SimpleMethod::GET, &url)
        .expect("url")
        .header(SimpleHeader::ACCEPT, "text/event-stream".to_string())
        .build();
    let response = client.send_async(req).await.expect("send_async");
    let (_, _, body) = response.into_parts();
    let SendSafeBody::SseStream(Some(mut iterator)) = body else {
        panic!("expected an SSE stream body");
    };
    let first = iterator.next().expect("data event").expect("parsed");
    assert_eq!(first.event.event_type(), Some("data"));
    assert_eq!(first.event.data(), Some("hello"));
    let second = iterator.next().expect("control event").expect("parsed");
    assert_eq!(second.event.event_type(), Some("control"));
    assert_eq!(
        second.event.data(),
        Some("{\"streamNextOffset\":\"00ff\",\"upToDate\":true}")
    );
    assert!(iterator.next().is_none(), "stream ends after the control");
}
