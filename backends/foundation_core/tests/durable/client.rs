//! Given/When/Then scenarios for the Durable Streams client core.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use proptest::prelude::*;

use foundation_core::durable::client::{
    ClientConfig, ClientError, ClientFuture, ClientRequest, ClientResponse, ClientSseStream,
    ClientTransport, DurableClient, DynamicHeaders, ReadMode, ReadResult, RetryPolicy, SseEvent,
    SseOpen, SseRead, TransportError,
};
use foundation_core::durable::StreamName;

#[derive(Clone, Default)]
struct SseHeadSpec {
    status: u16,
    headers: Vec<(String, String)>,
}

#[derive(Default)]
struct MockTransport {
    responses: Mutex<VecDeque<ClientResponse>>,
    /// Transport failures the test injects. A failure wins over a response.
    failures: Mutex<VecDeque<String>>,
    requests: Mutex<Vec<ClientRequest>>,
    events: Mutex<Vec<SseEvent>>,
    sse_head: Mutex<SseHeadSpec>,
}

impl MockTransport {
    fn with(responses: Vec<ClientResponse>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses.into()),
            failures: Mutex::new(VecDeque::new()),
            requests: Mutex::new(Vec::new()),
            events: Mutex::new(Vec::new()),
            sse_head: Mutex::new(SseHeadSpec {
                status: 200,
                headers: Vec::new(),
            }),
        })
    }

    fn with_sse_head(status: u16, headers: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(VecDeque::new()),
            failures: Mutex::new(VecDeque::new()),
            requests: Mutex::new(Vec::new()),
            events: Mutex::new(Vec::new()),
            sse_head: Mutex::new(SseHeadSpec {
                status,
                headers: headers
                    .iter()
                    .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                    .collect(),
            }),
        })
    }

    /// Inject transport failures, in order, before the queued responses.
    #[cfg(feature = "test-clock")]
    fn fail_next(&self, messages: &[&str]) {
        self.failures
            .lock()
            .expect("failures")
            .extend(messages.iter().map(|message| (*message).to_string()));
    }

    fn last_request(&self) -> ClientRequest {
        self.requests
            .lock()
            .expect("requests")
            .last()
            .cloned()
            .expect("a request")
    }
}

impl ClientTransport for MockTransport {
    fn send(&self, request: ClientRequest) -> ClientFuture<'_, ClientResponse> {
        self.requests.lock().expect("requests").push(request);
        if let Some(message) = self.failures.lock().expect("failures").pop_front() {
            return Box::pin(async move { Err(TransportError::new(message)) });
        }
        let response = self
            .responses
            .lock()
            .expect("responses")
            .pop_front()
            .unwrap_or(ClientResponse {
                status: 500,
                headers: BTreeMap::new(),
                body: Vec::new(),
            });
        Box::pin(async move { Ok(response) })
    }

    fn open_sse(&self, request: ClientRequest) -> ClientFuture<'_, SseOpen> {
        self.requests.lock().expect("requests").push(request);
        let events = self
            .events
            .lock()
            .expect("events")
            .drain(..)
            .collect::<Vec<_>>();
        let head = self.sse_head.lock().expect("sse head").clone();
        Box::pin(async move {
            let stream =
                futures_lite::stream::iter(events.into_iter().map(Ok::<_, TransportError>));
            let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for (key, value) in head.headers {
                headers.entry(key).or_default().push(value);
            }
            Ok(SseOpen {
                status: head.status,
                headers,
                stream: Box::pin(stream) as ClientSseStream,
            })
        })
    }
}

fn response(status: u16, headers: &[(&str, &str)], body: &[u8]) -> ClientResponse {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (key, value) in headers {
        map.entry((*key).to_string())
            .or_default()
            .push((*value).to_string());
    }
    ClientResponse {
        status,
        headers: map,
        body: body.to_vec(),
    }
}

fn client(transport: Arc<MockTransport>) -> DurableClient {
    DurableClient::new(
        transport,
        ClientConfig {
            base_url: "http://server".to_string(),
            default_headers: vec![("x-test".to_string(), "1".to_string())],
        },
    )
}

fn handle(transport: Arc<MockTransport>) -> foundation_core::durable::client::StreamHandle {
    client(transport).stream(&StreamName::parse("v1/stream/orders").expect("name"))
}

#[test]
fn given_a_client_when_it_creates_a_stream_then_it_sends_a_put_with_the_configuration() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            201,
            &[("stream-next-offset", "0000000000000003")],
            b"",
        )]);
        let result = handle(transport.clone())
            .create(
                Some("application/json"),
                Some(60),
                None,
                false,
                b"abc".to_vec(),
            )
            .await
            .expect("create");

        assert_eq!(result.status, 201);
        assert_eq!(result.offset.as_deref(), Some("0000000000000003"));

        let request = transport.last_request();
        assert_eq!(request.method.as_str(), "PUT");
        assert_eq!(request.url, "http://server/v1/stream/orders");
        assert!(request
            .headers
            .contains(&("content-type".to_string(), "application/json".to_string())));
        assert!(request
            .headers
            .contains(&("stream-ttl".to_string(), "60".to_string())));
        assert!(request
            .headers
            .contains(&("x-test".to_string(), "1".to_string())));
        assert_eq!(request.body, b"abc");
    });
}

#[test]
fn given_a_duplicate_producer_write_when_the_server_answers_204_then_it_is_a_duplicate() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(204, &[], b"")]);
        let result = handle(transport.clone())
            .append(
                b"x".to_vec(),
                Some("text/plain"),
                None,
                Some(("p1", 0, 0)),
                false,
            )
            .await
            .expect("append");

        assert_eq!(result.status, 204);
        assert!(result.duplicate);

        let request = transport.last_request();
        assert_eq!(request.method.as_str(), "POST");
        assert!(request
            .headers
            .contains(&("producer-id".to_string(), "p1".to_string())));
    });
}

#[test]
fn given_a_text_stream_when_a_catch_up_read_runs_then_it_returns_one_chunk() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            200,
            &[
                ("content-type", "text/plain"),
                ("stream-next-offset", "0000000000000005"),
                ("stream-up-to-date", "true"),
            ],
            b"hello",
        )]);
        let result = handle(transport.clone())
            .read()
            .offset("-1")
            .send()
            .await
            .expect("read");

        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].data, b"hello");
        assert_eq!(result.next_offset.as_deref(), Some("0000000000000005"));
        assert!(result.up_to_date);
        assert_eq!(
            transport.last_request().url,
            "http://server/v1/stream/orders?offset=-1"
        );
    });
}

#[test]
fn given_a_json_stream_when_a_read_runs_then_it_returns_one_chunk_per_message() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            200,
            &[("content-type", "application/json")],
            br#"[{"n":1},{"n":2}]"#,
        )]);
        let result = handle(transport).read().send().await.expect("read");

        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].data, br#"[{"n":1},{"n":2}]"#);
    });
}

#[test]
fn given_a_long_poll_timeout_when_the_server_answers_204_then_the_client_is_up_to_date() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            204,
            &[
                ("stream-next-offset", "0000000000000005"),
                ("stream-up-to-date", "true"),
            ],
            b"",
        )]);
        let result = handle(transport.clone())
            .read()
            .mode(ReadMode::LongPoll)
            .offset("0000000000000005")
            .send()
            .await
            .expect("read");

        assert_eq!(result.status, 204);
        assert_eq!(
            result.chunks,
            [] as [foundation_core::durable::client::ReadChunk; 0]
        );
        assert!(result.up_to_date);
        assert_eq!(
            transport.last_request().url,
            "http://server/v1/stream/orders?offset=0000000000000005&live=long-poll"
        );
    });
}

#[test]
fn given_a_head_response_when_head_runs_then_it_reports_the_tail_and_closure() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            200,
            &[
                ("stream-next-offset", "0000000000000009"),
                ("content-type", "text/plain"),
                ("stream-ttl", "30"),
                ("stream-closed", "true"),
            ],
            b"",
        )]);
        let head = handle(transport.clone()).head().await.expect("head");

        assert_eq!(head.offset.as_deref(), Some("0000000000000009"));
        assert_eq!(head.content_type.as_deref(), Some("text/plain"));
        assert_eq!(head.ttl_seconds, Some(30));
        assert!(head.closed);
        assert_eq!(transport.last_request().method.as_str(), "HEAD");
    });
}

#[test]
fn given_a_missing_stream_when_delete_runs_then_the_error_names_the_path() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(404, &[], b"")]);
        let error = handle(transport).delete().await.expect_err("missing");

        match error {
            ClientError::NotFound { path } => assert_eq!(path, "/v1/stream/orders"),
            other => panic!("unexpected error: {other}"),
        }
    });
}

#[test]
fn given_a_closed_stream_when_append_runs_then_the_error_is_closed() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            409,
            &[("stream-closed", "true")],
            b"stream is closed",
        )]);
        let error = handle(transport)
            .append(b"x".to_vec(), Some("text/plain"), None, None, false)
            .await
            .expect_err("closed");

        assert!(matches!(error, ClientError::Closed));
    });
}

#[test]
fn given_a_malformed_json_body_when_a_read_runs_then_the_error_is_a_parse_error() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            200,
            &[("content-type", "application/json")],
            b"{",
        )]);
        let error = handle(transport).read().send().await.expect_err("parse");
        assert!(matches!(error, ClientError::Parse(_)));
    });
}

#[test]
fn given_a_bad_status_when_a_read_runs_then_the_error_carries_the_status() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(503, &[], b"down")]);
        let error = handle(transport).read().send().await.expect_err("status");
        match error {
            ClientError::UnexpectedStatus { status, body, .. } => {
                assert_eq!(status, 503);
                assert_eq!(body, "down");
            }
            other => panic!("unexpected error: {other}"),
        }
    });
}

fn sse(event_type: &str, data: &str) -> SseEvent {
    SseEvent {
        event_type: Some(event_type.to_string()),
        data: data.to_string(),
    }
}

/// Consume an open Server-Sent Events (SSE) read to its end and fold it into
/// a [`ReadResult`]. The mock events are finite, so the stream always ends.
async fn drain(mut read: SseRead) -> ReadResult {
    let mut result = ReadResult {
        status: read.status(),
        ..ReadResult::default()
    };
    while let Some(chunk) = read.next().await.expect("sse item") {
        if !chunk.data.is_empty() {
            result.chunks.push(chunk);
        }
        result.next_offset = read.next_offset().map(str::to_string);
        result.cursor = read.cursor().map(str::to_string);
        result.up_to_date = read.up_to_date();
        result.closed = read.closed();
    }
    result
}

#[test]
fn given_a_sse_data_and_control_event_when_a_read_runs_then_it_reports_the_chunk_and_offset() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![]);
        transport.events.lock().expect("events").extend([
            sse("data", "hello"),
            sse(
                "control",
                r#"{"streamNextOffset":"00ff","streamCursor":"7","upToDate":true}"#,
            ),
        ]);
        let read = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .open_sse()
            .await
            .expect("sse");
        let result = drain(read).await;

        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].data, b"hello");
        assert_eq!(result.next_offset.as_deref(), Some("00ff"));
        assert_eq!(result.cursor.as_deref(), Some("7"));
        assert!(result.up_to_date);
    });
}

#[test]
fn given_a_numeric_cursor_from_an_old_peer_when_a_read_runs_then_it_still_reports_it() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![]);
        transport.events.lock().expect("events").extend([
            sse("data", "hello"),
            sse(
                "control",
                r#"{"streamNextOffset":"00ff","streamCursor":7,"upToDate":true}"#,
            ),
        ]);
        let read = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .open_sse()
            .await
            .expect("sse");
        let result = drain(read).await;

        assert_eq!(
            result.cursor.as_deref(),
            Some("7"),
            "a numeric cursor from an old peer is still read"
        );
    });
}

#[test]
fn given_a_base64_sse_payload_when_a_binary_read_runs_then_it_decodes_the_bytes() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![]);
        transport.events.lock().expect("events").extend([
            sse("data", "SGVsbG8="),
            sse(
                "control",
                r#"{"streamNextOffset":"00ff","streamClosed":true}"#,
            ),
        ]);
        let read = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .sse_binary(true)
            .open_sse()
            .await
            .expect("sse");
        let result = drain(read).await;

        assert_eq!(result.chunks[0].data, b"Hello");
        assert!(result.closed);
    });
}

#[test]
fn given_a_control_event_with_no_type_when_a_read_runs_then_it_still_ends_the_read() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![]);
        transport.events.lock().expect("events").push(SseEvent {
            event_type: None,
            data: r#"{"streamNextOffset":"00aa","upToDate":true}"#.to_string(),
        });
        let read = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .open_sse()
            .await
            .expect("sse");
        let result = drain(read).await;

        assert_eq!(
            result.chunks,
            [] as [foundation_core::durable::client::ReadChunk; 0]
        );
        assert_eq!(result.next_offset.as_deref(), Some("00aa"));
    });
}

#[test]
fn given_an_empty_data_event_when_a_read_runs_then_it_adds_no_chunk() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![]);
        transport.events.lock().expect("events").extend([
            sse("data", ""),
            sse("control", r#"{"streamNextOffset":"00bb","upToDate":true}"#),
        ]);
        let read = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .open_sse()
            .await
            .expect("sse");
        let result = drain(read).await;

        assert_eq!(
            result.chunks,
            [] as [foundation_core::durable::client::ReadChunk; 0]
        );
    });
}

#[test]
fn given_a_missing_stream_when_an_sse_read_runs_then_the_head_answers_not_found() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with_sse_head(404, &[]);
        let error = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .open_sse()
            .await
            .err()
            .expect("missing");
        assert!(matches!(error, ClientError::NotFound { .. }));
    });
}

#[test]
fn given_an_sse_encoding_header_when_a_text_read_runs_then_the_data_is_decoded() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport =
            MockTransport::with_sse_head(200, &[("stream-sse-data-encoding", "base64")]);
        transport.events.lock().expect("events").extend([
            sse("data", "SGVsbG8="),
            sse("control", r#"{"streamNextOffset":"00cc"}"#),
        ]);
        let read = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .open_sse()
            .await
            .expect("sse");
        let result = drain(read).await;

        assert_eq!(result.chunks[0].data, b"Hello");
    });
}

#[test]
fn given_a_multiline_base64_event_when_it_is_read_then_it_decodes_to_the_text() {
    foundation_core::valtron::block_on_future_scoped(async {
        use base64::Engine as _;
        let text = "Line 1\nLine 2\nLine 3";
        let encoded = base64::engine::general_purpose::STANDARD.encode(text);
        let transport =
            MockTransport::with_sse_head(200, &[("stream-sse-data-encoding", "base64")]);
        transport.events.lock().expect("events").extend([
            sse("data", &encoded),
            sse("control", r#"{"streamNextOffset":"00dd","upToDate":true}"#),
        ]);
        let read = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .open_sse()
            .await
            .expect("sse");
        let result = drain(read).await;

        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].data, text.as_bytes());
    });
}

#[test]
fn given_a_large_base64_payload_split_across_lines_when_it_is_read_then_it_decodes_exactly() {
    foundation_core::valtron::block_on_future_scoped(async {
        use base64::Engine as _;
        // A 4096-byte pattern: the bytes 0 to 255 repeated sixteen times.
        let original: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let mut encoded = base64::engine::general_purpose::STANDARD.encode(&original);
        // A server splits a long payload across `data:` lines. The parser
        // joins those lines with a newline, so insert one every 76 characters.
        let mut split = String::new();
        for (index, character) in encoded.chars().enumerate() {
            if index > 0 && index % 76 == 0 {
                split.push('\n');
            }
            split.push(character);
        }
        encoded = split;

        let transport =
            MockTransport::with_sse_head(200, &[("stream-sse-data-encoding", "base64")]);
        transport.events.lock().expect("events").extend([
            sse("data", &encoded),
            sse("control", r#"{"streamNextOffset":"00ee","upToDate":true}"#),
        ]);
        let read = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .open_sse()
            .await
            .expect("sse");
        let result = drain(read).await;

        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].data, original);
    });
}

#[test]
fn given_a_closed_create_when_it_runs_then_the_closed_header_is_sent() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(201, &[], b"")]);
        handle(transport.clone())
            .create(Some("text/plain"), None, None, true, Vec::new())
            .await
            .expect("create");

        assert!(transport
            .last_request()
            .headers
            .contains(&("stream-closed".to_string(), "true".to_string())));
    });
}

#[test]
fn given_a_close_when_it_runs_then_it_sends_the_closed_header_and_returns_the_offset() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            200,
            &[("stream-next-offset", "0000000000000009")],
            b"",
        )]);
        let result = handle(transport.clone())
            .close(b"final".to_vec(), Some("text/plain"))
            .await
            .expect("close");

        assert_eq!(result.offset.as_deref(), Some("0000000000000009"));
        let request = transport.last_request();
        assert_eq!(request.method.as_str(), "POST");
        assert!(request
            .headers
            .contains(&("stream-closed".to_string(), "true".to_string())));
        assert_eq!(request.body, b"final");
    });
}

#[test]
fn given_a_stream_sequence_when_an_append_runs_then_the_header_is_sent() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(204, &[], b"")]);
        handle(transport.clone())
            .append(b"x".to_vec(), Some("text/plain"), Some("0007"), None, false)
            .await
            .expect("append");

        assert!(transport
            .last_request()
            .headers
            .contains(&("stream-seq".to_string(), "0007".to_string())));
    });
}

#[test]
fn given_an_absolute_deadline_when_head_runs_then_it_reports_the_timestamp() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            200,
            &[("stream-expires-at", "2030-01-01T00:00:00Z")],
            b"",
        )]);
        let head = handle(transport).head().await.expect("head");
        assert_eq!(head.expires_at.as_deref(), Some("2030-01-01T00:00:00Z"));
    });
}

#[test]
fn given_a_cursor_when_a_read_runs_then_it_is_echoed_on_the_query() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(200, &[], b"")]);
        handle(transport.clone())
            .read()
            .cursor("42")
            .send()
            .await
            .expect("read");
        assert!(transport.last_request().url.ends_with("cursor=42"));
    });
}

#[test]
fn given_a_matching_etag_when_a_read_runs_then_it_reports_not_modified() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(304, &[], b"")]);
        let result = handle(transport.clone())
            .read()
            .if_none_match("\"tag\"")
            .send()
            .await
            .expect("conditional read");
        assert_eq!(result.status, 304);
        assert_eq!(
            result.chunks,
            [] as [foundation_core::durable::client::ReadChunk; 0]
        );
        assert!(result.up_to_date);
        assert!(transport
            .last_request()
            .headers
            .contains(&("if-none-match".to_string(), "\"tag\"".to_string())));
    });
}

#[test]
fn given_a_closed_stream_at_the_tail_when_a_read_runs_then_it_reports_the_end() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            200,
            &[
                ("stream-closed", "true"),
                ("stream-up-to-date", "true"),
                ("stream-next-offset", "0000000000000003"),
            ],
            b"",
        )]);
        let result = handle(transport)
            .read()
            .offset("0000000000000003")
            .send()
            .await
            .expect("read");

        assert_eq!(
            result.chunks,
            [] as [foundation_core::durable::client::ReadChunk; 0]
        );
        assert!(result.closed);
        assert!(result.up_to_date);
        assert_eq!(result.next_offset.as_deref(), Some("0000000000000003"));
    });
}

#[test]
fn given_an_absolute_deadline_when_create_runs_then_the_header_is_sent() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(201, &[], b"")]);
        handle(transport.clone())
            .create(
                Some("text/plain"),
                None,
                Some("2030-01-01T00:00:00Z"),
                false,
                Vec::new(),
            )
            .await
            .expect("create");

        assert!(transport.last_request().headers.contains(&(
            "stream-expires-at".to_string(),
            "2030-01-01T00:00:00Z".to_string(),
        )));
    });
}

#[foundation_core::valtron::valtron_test(timeout = 5000)]
fn given_a_client_reader_when_pages_arrive_then_it_yields_them_in_order() {
    use foundation_core::durable::valtron::ClientReader;
    use foundation_core::valtron::{Stream, StreamSpread};

    let transport = MockTransport::with(vec![
        response(200, &[("stream-next-offset", "01")], b"ab"),
        response(
            200,
            &[("stream-next-offset", "02"), ("stream-up-to-date", "true")],
            b"cd",
        ),
    ]);
    let reader = ClientReader::new(handle(transport), foundation_context::Ctx::host());

    let mut bytes = Vec::new();
    for item in reader {
        match item {
            Stream::Next(data) => bytes.extend_from_slice(&data),
            Stream::Spread(items) => {
                for item in items {
                    if let StreamSpread::Done(data) = item {
                        bytes.extend_from_slice(&data);
                    }
                }
            }
            _ => {}
        }
    }
    assert_eq!(bytes, b"abcd");
}

// ---- Retry, producer and dynamic-header scenarios (issue #1135) ----

/// A timer that records each deadline and resolves at once.
#[derive(Default)]
struct RecordingTimer {
    calls: Mutex<Vec<(foundation_context::Instant, foundation_context::Instant)>>,
}

impl RecordingTimer {
    /// The requested waits, measured against the clock that produced them.
    fn delays(&self) -> Vec<foundation_context::Duration> {
        self.calls
            .lock()
            .expect("timer calls")
            .iter()
            .map(|(now, deadline)| deadline.saturating_duration_since(*now))
            .collect()
    }
}

impl foundation_core::env::Timer for RecordingTimer {
    fn sleep_until(
        &self,
        ctx: foundation_context::Ctx,
        deadline: foundation_context::Instant,
    ) -> foundation_core::env::TimerFuture {
        self.calls
            .lock()
            .expect("timer calls")
            .push((ctx.clock().now(), deadline));
        Box::pin(async {})
    }
}

/// A context with a real monotonic clock and a timer that never blocks.
fn retry_ctx() -> (foundation_context::Ctx, std::sync::Arc<RecordingTimer>) {
    let clock: std::sync::Arc<dyn foundation_context::Clock> =
        std::sync::Arc::new(foundation_context::MonotonicClock::new());
    let ctx = foundation_context::Ctx::fixed(clock);
    let timer = std::sync::Arc::new(RecordingTimer::default());
    ctx.store(foundation_core::env::Env::new(
        timer.clone(),
        std::sync::Arc::new(foundation_core::env::SeededEntropy::new(9)),
        std::sync::Arc::new(foundation_core::env::CounterSource::new(1)),
    ));
    (ctx, timer)
}

fn retrying_client(
    transport: std::sync::Arc<MockTransport>,
    ctx: foundation_context::Ctx,
    policy: RetryPolicy,
) -> DurableClient {
    DurableClient::new(
        transport,
        ClientConfig {
            base_url: "http://server".to_string(),
            default_headers: Vec::new(),
        },
    )
    .with_ctx(ctx)
    .with_retry_policy(policy)
}

fn header(request: &ClientRequest, name: &str) -> Option<String> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
}

/// A provider whose header value counts the attempts.
#[derive(Default)]
struct CounterHeader {
    next: std::sync::atomic::AtomicU64,
}

impl DynamicHeaders for CounterHeader {
    fn headers(&self) -> Vec<(String, String)> {
        let value = self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        vec![("x-attempt".to_string(), value.to_string())]
    }
}

#[test]
fn given_a_429_with_retry_after_when_a_read_runs_then_it_waits_and_retries() {
    foundation_core::valtron::block_on_future_scoped(async {
        let (ctx, timer) = retry_ctx();
        let transport = MockTransport::with(vec![
            response(429, &[("retry-after", "1")], b""),
            response(200, &[("content-type", "text/plain")], b"ok"),
        ]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default().jitter(0.0));
        let result = client
            .stream(&StreamName::parse("v1/stream/orders").expect("name"))
            .read()
            .send()
            .await
            .expect("a retried read");

        assert_eq!(result.chunks[0].data, b"ok");
        assert_eq!(transport.requests.lock().expect("requests").len(), 2);
        let delays = timer.delays();
        assert_eq!(delays.len(), 1, "one wait before the retry");
        assert!(
            delays[0] >= foundation_context::Duration::from_millis(999)
                && delays[0] <= foundation_context::Duration::from_secs(1),
            "the wait is the server Retry-After value, got {:?}",
            delays[0]
        );
    });
}

#[test]
fn given_a_409_when_a_read_runs_then_it_is_not_retried() {
    foundation_core::valtron::block_on_future_scoped(async {
        let (ctx, timer) = retry_ctx();
        let transport = MockTransport::with(vec![response(409, &[], b"conflict")]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default());
        let error = client
            .stream(&StreamName::parse("v1/stream/orders").expect("name"))
            .read()
            .send()
            .await
            .expect_err("a conflict");

        assert!(matches!(
            error,
            ClientError::UnexpectedStatus { status: 409, .. }
        ));
        assert_eq!(transport.requests.lock().expect("requests").len(), 1);
        assert_eq!(
            timer.delays(),
            [] as [foundation_context::Duration; 0],
            "no wait is needed"
        );
    });
}

#[test]
fn does_not_retry_404() {
    foundation_core::valtron::block_on_future_scoped(async {
        let (ctx, timer) = retry_ctx();
        let transport = MockTransport::with(vec![response(404, &[], b"")]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default());
        let error = client
            .stream(&StreamName::parse("v1/stream/missing").expect("name"))
            .read()
            .send()
            .await
            .expect_err("missing");

        assert!(matches!(error, ClientError::NotFound { .. }));
        assert_eq!(transport.requests.lock().expect("requests").len(), 1);
        assert_eq!(
            timer.delays(),
            [] as [foundation_context::Duration; 0],
            "no wait is needed"
        );
    });
}

#[test]
fn retry_exhaustion_is_an_error() {
    foundation_core::valtron::block_on_future_scoped(async {
        let (ctx, timer) = retry_ctx();
        let transport = MockTransport::with(vec![
            response(500, &[], b"one"),
            response(500, &[], b"two"),
            response(500, &[], b"three"),
        ]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default().jitter(0.0));
        let error = client
            .stream(&StreamName::parse("v1/stream/orders").expect("name"))
            .read()
            .send()
            .await
            .expect_err("an exhausted read");

        match error {
            ClientError::RetryExhausted {
                attempts,
                status,
                body,
                ..
            } => {
                assert_eq!(attempts, 3);
                assert_eq!(status, Some(500));
                assert_eq!(body, "three");
            }
            other => panic!("unexpected error: {other}"),
        }
        assert_eq!(transport.requests.lock().expect("requests").len(), 3);
        assert_eq!(timer.delays().len(), 2, "two waits between three attempts");
    });
}

#[test]
fn given_a_producer_when_two_appends_run_then_the_sequences_advance() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(200, &[], b""), response(200, &[], b"")]);
        let producer = handle(transport.clone())
            .producer("p1")
            .build()
            .expect("a producer");
        producer
            .append(b"a".to_vec(), Some("text/plain"))
            .await
            .expect("first append");
        producer
            .append(b"b".to_vec(), Some("text/plain"))
            .await
            .expect("second append");

        let requests = transport.requests.lock().expect("requests");
        assert_eq!(header(&requests[0], "producer-id").as_deref(), Some("p1"));
        assert_eq!(header(&requests[0], "producer-epoch").as_deref(), Some("0"));
        assert_eq!(header(&requests[0], "producer-seq").as_deref(), Some("0"));
        assert_eq!(header(&requests[1], "producer-seq").as_deref(), Some("1"));
        assert_eq!(producer.sequence(), 2);
    });
}

#[test]
fn producer_deduplicates_retry() {
    foundation_core::valtron::block_on_future_scoped(async {
        let (ctx, _timer) = retry_ctx();
        let transport =
            MockTransport::with(vec![response(500, &[], b"down"), response(204, &[], b"")]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default());
        let producer = client
            .handle("/v1/stream/orders")
            .producer("p1")
            .build()
            .expect("a producer");
        let result = producer
            .append(b"x".to_vec(), Some("text/plain"))
            .await
            .expect("a retried append");

        assert!(result.duplicate, "a 204 is a duplicate");
        let requests = transport.requests.lock().expect("requests");
        assert_eq!(requests.len(), 2);
        assert_eq!(
            header(&requests[0], "producer-seq"),
            header(&requests[1], "producer-seq"),
            "a retry repeats the sequence"
        );
        assert_eq!(header(&requests[1], "producer-seq").as_deref(), Some("0"));
        assert_eq!(producer.sequence(), 1, "the sequence advances once");
    });
}

#[test]
fn auto_claim_moves_epoch() {
    foundation_core::valtron::block_on_future_scoped(async {
        let (ctx, _timer) = retry_ctx();
        let transport = MockTransport::with(vec![
            response(403, &[("producer-epoch", "5")], b"fenced"),
            response(200, &[], b""),
        ]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default());
        let producer = client
            .handle("/v1/stream/orders")
            .producer("p1")
            .epoch(0)
            .auto_claim(true)
            .build()
            .expect("a producer");
        let result = producer
            .append(b"recovered".to_vec(), Some("text/plain"))
            .await
            .expect("an auto-claimed append");

        assert_eq!(result.status, 200);
        let requests = transport.requests.lock().expect("requests");
        assert_eq!(header(&requests[0], "producer-epoch").as_deref(), Some("0"));
        assert_eq!(header(&requests[1], "producer-epoch").as_deref(), Some("6"));
        assert_eq!(header(&requests[1], "producer-seq").as_deref(), Some("0"));
        assert_eq!(producer.epoch(), 6);
    });
}

#[test]
fn dynamic_header_counter() {
    foundation_core::valtron::block_on_future_scoped(async {
        let (ctx, _timer) = retry_ctx();
        let transport = MockTransport::with(vec![
            response(500, &[], b"down"),
            response(200, &[("content-type", "text/plain")], b"ok"),
        ]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default())
            .with_dynamic_headers(std::sync::Arc::new(CounterHeader::default()));
        client
            .handle("/v1/stream/orders")
            .read()
            .send()
            .await
            .expect("a retried read");

        let requests = transport.requests.lock().expect("requests");
        assert_eq!(header(&requests[0], "x-attempt").as_deref(), Some("1"));
        assert_eq!(
            header(&requests[1], "x-attempt").as_deref(),
            Some("2"),
            "the provider runs once per attempt"
        );
    });
}

#[test]
fn detach_does_not_close() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(Vec::new());
        let producer = handle(transport.clone())
            .producer("p1")
            .build()
            .expect("a producer");
        producer.detach();

        assert!(producer.is_detached());
        let error = producer
            .append(b"x".to_vec(), Some("text/plain"))
            .await
            .expect_err("a detached producer");
        assert!(matches!(error, ClientError::Detached));
        assert_eq!(
            transport.requests.lock().expect("requests").len(),
            0,
            "detach and a refused append send nothing"
        );
    });
}

#[cfg(feature = "test-clock")]
#[test]
fn given_a_simulated_clock_when_a_retry_runs_then_the_clock_sets_the_wait() {
    use foundation_context::Clock as _;
    use foundation_core::valtron::SimulatedClock;

    foundation_core::valtron::block_on_future_scoped(async {
        let clock = std::sync::Arc::new(SimulatedClock::new(foundation_context::Duration::ZERO));
        let ctx = foundation_context::Ctx::fixed(clock.clone());
        let timer = std::sync::Arc::new(RecordingTimer::default());
        ctx.store(foundation_core::env::Env::new(
            timer.clone(),
            std::sync::Arc::new(foundation_core::env::SeededEntropy::new(11)),
            std::sync::Arc::new(foundation_core::env::CounterSource::new(1)),
        ));
        let transport =
            MockTransport::with(vec![response(500, &[], b"down"), response(200, &[], b"")]);
        let client = retrying_client(
            transport,
            ctx,
            RetryPolicy::default()
                .initial_backoff(foundation_context::Duration::from_millis(250))
                .jitter(0.0),
        );
        client
            .handle("/v1/stream/orders")
            .read()
            .send()
            .await
            .expect("a retried read");

        assert_eq!(
            timer.delays(),
            vec![foundation_context::Duration::from_millis(250)]
        );
        assert_eq!(
            clock.elapsed(),
            foundation_context::Duration::ZERO,
            "the wait came from the simulated clock, not real time"
        );
    });
}

// ---- Producer pipelining and linger scenarios (issue #1152) ----

use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context as TaskContext, Poll};

/// A transport that keeps every request pending for one poll pass, and records
/// how many requests were pending at the same time.
struct CountingTransport {
    responses: Mutex<VecDeque<ClientResponse>>,
    requests: Mutex<Vec<ClientRequest>>,
    outstanding: AtomicUsize,
    max_outstanding: AtomicUsize,
}

impl CountingTransport {
    fn with(responses: Vec<ClientResponse>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(Vec::new()),
            outstanding: AtomicUsize::new(0),
            max_outstanding: AtomicUsize::new(0),
        })
    }
}

impl ClientTransport for CountingTransport {
    fn send(&self, request: ClientRequest) -> ClientFuture<'_, ClientResponse> {
        self.requests.lock().expect("requests").push(request);
        let response = self
            .responses
            .lock()
            .expect("responses")
            .pop_front()
            .unwrap_or(ClientResponse {
                status: 500,
                headers: BTreeMap::new(),
                body: Vec::new(),
            });
        let mut polled = false;
        Box::pin(futures_lite::future::poll_fn(
            move |cx: &mut TaskContext<'_>| {
                if !polled {
                    polled = true;
                    let now = self.outstanding.fetch_add(1, Ordering::SeqCst) + 1;
                    self.max_outstanding.fetch_max(now, Ordering::SeqCst);
                    // Return at once on the next poll so the test never waits
                    // on a real timer.
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                self.outstanding.fetch_sub(1, Ordering::SeqCst);
                Poll::Ready(Ok(response.clone()))
            },
        ))
    }
}

/// A context with a simulated clock and a timer that records each deadline.
#[cfg(feature = "test-clock")]
fn simulated_ctx(
    seed: u64,
) -> (
    foundation_context::Ctx,
    std::sync::Arc<RecordingTimer>,
    std::sync::Arc<foundation_core::valtron::SimulatedClock>,
) {
    let clock = std::sync::Arc::new(foundation_core::valtron::SimulatedClock::new(
        foundation_context::Duration::ZERO,
    ));
    let ctx = foundation_context::Ctx::fixed(clock.clone());
    let timer = std::sync::Arc::new(RecordingTimer::default());
    ctx.store(foundation_core::env::Env::new(
        timer.clone(),
        std::sync::Arc::new(foundation_core::env::SeededEntropy::new(seed)),
        std::sync::Arc::new(foundation_core::env::CounterSource::new(1)),
    ));
    (ctx, timer, clock)
}

#[cfg(feature = "test-clock")]
#[test]
fn several_appends_during_linger_become_one_batch() {
    use foundation_context::Clock as _;

    let (ctx, timer, clock) = simulated_ctx(21);
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            200,
            &[("stream-next-offset", "0000000000000003")],
            b"",
        )]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default());
        let producer = client
            .handle("/v1/stream/orders")
            .producer("p1")
            .build()
            .expect("a producer");
        let second = producer.clone();
        let third = producer.clone();
        let (first, (second, third)) = futures_lite::future::zip(
            producer.append(b"a".to_vec(), Some("text/plain")),
            futures_lite::future::zip(
                second.append(b"b".to_vec(), Some("text/plain")),
                third.append(b"c".to_vec(), Some("text/plain")),
            ),
        )
        .await;

        for result in [first, second, third] {
            let outcome = result.expect("a batched append");
            assert_eq!(outcome.offset.as_deref(), Some("0000000000000003"));
        }
        let requests = transport.requests.lock().expect("requests");
        assert_eq!(requests.len(), 1, "the three appends share one request");
        assert_eq!(header(&requests[0], "producer-seq").as_deref(), Some("0"));
        assert_eq!(requests[0].body, b"abc");
        assert_eq!(
            timer.delays(),
            vec![foundation_context::Duration::from_millis(5)],
            "the default linger waits five milliseconds"
        );
        assert_eq!(
            clock.elapsed(),
            foundation_context::Duration::ZERO,
            "the wait came from the simulated clock, not real time"
        );
    });
}

#[test]
fn max_in_flight_bounds_concurrent_requests() {
    foundation_core::valtron::block_on_future_scoped(async {
        let responses = (0..5).map(|_| response(200, &[], b"")).collect();
        let transport = CountingTransport::with(responses);
        let client = DurableClient::new(
            transport.clone(),
            ClientConfig {
                base_url: "http://server".to_string(),
                default_headers: Vec::new(),
            },
        );
        let producer = client
            .handle("/v1/stream/orders")
            .producer("p1")
            .max_batch_items(1)
            .max_in_flight(2)
            .build()
            .expect("a producer");
        let items = vec![
            b"a".to_vec(),
            b"b".to_vec(),
            b"c".to_vec(),
            b"d".to_vec(),
            b"e".to_vec(),
        ];
        let results = producer
            .append_batch(items, Some("text/plain"))
            .await
            .expect("five batches");

        assert_eq!(results.len(), 5);
        assert_eq!(transport.requests.lock().expect("requests").len(), 5);
        assert_eq!(
            transport.max_outstanding.load(Ordering::SeqCst),
            2,
            "never more than max_in_flight requests are in flight"
        );
    });
}

#[test]
fn a_failed_batch_retries_with_the_same_sequence() {
    foundation_core::valtron::block_on_future_scoped(async {
        let (ctx, _timer) = retry_ctx();
        let transport =
            MockTransport::with(vec![response(500, &[], b"down"), response(200, &[], b"")]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default());
        let producer = client
            .handle("/v1/stream/orders")
            .producer("p1")
            .build()
            .expect("a producer");
        let results = producer
            .append_batch(vec![b"x".to_vec()], Some("text/plain"))
            .await
            .expect("a retried batch");

        assert_eq!(results.len(), 1);
        let requests = transport.requests.lock().expect("requests");
        assert_eq!(requests.len(), 2);
        assert_eq!(
            header(&requests[0], "producer-seq"),
            header(&requests[1], "producer-seq"),
            "the retry keeps the sequence"
        );
        assert_eq!(header(&requests[1], "producer-seq").as_deref(), Some("0"));
        assert_eq!(producer.sequence(), 1, "the sequence advances once");
    });
}

#[test]
fn pipelined_batches_preserve_wire_and_acknowledgement_order() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![
            response(200, &[("stream-next-offset", "01")], b""),
            response(200, &[("stream-next-offset", "02")], b""),
            response(200, &[("stream-next-offset", "03")], b""),
        ]);
        let producer = handle(transport.clone())
            .producer("p1")
            .max_batch_items(1)
            .max_in_flight(3)
            .build()
            .expect("a producer");
        let results = producer
            .append_batch(
                vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
                Some("text/plain"),
            )
            .await
            .expect("three batches");

        let offsets: Vec<Option<&str>> = results
            .iter()
            .map(|result| result.offset.as_deref())
            .collect();
        assert_eq!(offsets, vec![Some("01"), Some("02"), Some("03")]);
        let requests = transport.requests.lock().expect("requests");
        assert_eq!(requests.len(), 3);
        for (index, request) in requests.iter().enumerate() {
            assert_eq!(
                header(request, "producer-seq"),
                Some(index.to_string()),
                "the wire keeps the sequence order"
            );
            assert_eq!(
                request.body,
                vec![b'a' + u8::try_from(index).expect("a small index")]
            );
        }
    });
}

#[cfg(feature = "test-clock")]
#[test]
fn given_a_deterministic_clock_when_linger_runs_then_the_clock_sets_the_wait() {
    use foundation_context::Clock as _;

    let (ctx, timer, clock) = simulated_ctx(23);
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(200, &[], b"")]);
        let client = retrying_client(transport, ctx, RetryPolicy::default());
        let producer = client
            .handle("/v1/stream/orders")
            .producer("p1")
            .linger(foundation_context::Duration::from_millis(25))
            .build()
            .expect("a producer");
        producer
            .append(b"x".to_vec(), Some("text/plain"))
            .await
            .expect("an append");

        assert_eq!(
            timer.delays(),
            vec![foundation_context::Duration::from_millis(25)],
            "linger uses the injected clock"
        );
        assert_eq!(
            clock.elapsed(),
            foundation_context::Duration::ZERO,
            "the wait came from the simulated clock, not real time"
        );
    });
}

// ---- Property and failure-injection oracles (feature 07 T12) ----

/// A transport that acts as a small in-memory Durable Streams server.
///
/// It answers the six client verbs with byte-exact bodies and fixed-width
/// hexadecimal offsets, so the property test checks the client and not a mock
/// script.
#[derive(Default)]
struct MemoryServer {
    state: Mutex<MemoryState>,
}

#[derive(Default)]
struct MemoryState {
    log: Vec<u8>,
    appended: usize,
}

impl MemoryServer {
    fn offset(value: usize) -> String {
        format!("{value:016x}")
    }

    fn respond(&self, request: &ClientRequest) -> ClientResponse {
        let mut state = self.state.lock().expect("memory state");
        match request.method.as_str() {
            "PUT" => {
                state.log.clone_from(&request.body);
                let offset = Self::offset(state.log.len());
                response(201, &[("stream-next-offset", &offset)], b"")
            }
            "POST" => {
                state.log.extend_from_slice(&request.body);
                state.appended += 1;
                let offset = Self::offset(state.log.len());
                response(204, &[("stream-next-offset", &offset)], b"")
            }
            "GET" => {
                let offset = Self::offset(state.log.len());
                response(
                    200,
                    &[
                        ("content-type", "application/octet-stream"),
                        ("stream-next-offset", &offset),
                        ("stream-up-to-date", "true"),
                    ],
                    &state.log,
                )
            }
            "HEAD" => {
                let offset = Self::offset(state.log.len());
                response(200, &[("stream-next-offset", &offset)], b"")
            }
            _ => response(204, &[], b""),
        }
    }

    fn appended(&self) -> usize {
        self.state.lock().expect("memory state").appended
    }
}

impl ClientTransport for MemoryServer {
    fn send(&self, request: ClientRequest) -> ClientFuture<'_, ClientResponse> {
        let response = self.respond(&request);
        Box::pin(async move { Ok(response) })
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Append a random sequence of chunks through the client, read it back, and
    /// check the bytes and the offsets. The bounds keep the run fast.
    #[test]
    fn given_random_chunks_when_the_client_appends_and_reads_then_bytes_are_exact(
        chunks in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..=64), 1..=32),
    ) {
        let (read_bytes, model, offsets, up_to_date, read_offset, appends) =
            foundation_core::valtron::block_on_future_scoped(async {
                let transport = Arc::new(MemoryServer::default());
                let client = DurableClient::new(
                    transport.clone(),
                    ClientConfig {
                        base_url: "http://server".to_string(),
                        default_headers: Vec::new(),
                    },
                );
                let handle = client.stream(&StreamName::parse("property/client").expect("name"));
                handle
                    .create(
                        Some("application/octet-stream"),
                        None,
                        None,
                        false,
                        Vec::new(),
                    )
                    .await
                    .expect("create");

                let mut model = Vec::new();
                let mut offsets = Vec::new();
                for chunk in &chunks {
                    let outcome = handle
                        .append(
                            chunk.clone(),
                            Some("application/octet-stream"),
                            None,
                            None,
                            false,
                        )
                        .await
                        .expect("append");
                    offsets.push(outcome.offset.expect("an append offset"));
                    model.extend_from_slice(chunk);
                }

                let read = handle.read().send().await.expect("catch-up read");
                let read_bytes: Vec<u8> = read
                    .chunks
                    .iter()
                    .flat_map(|chunk| chunk.data.iter().copied())
                    .collect();
                (
                    read_bytes,
                    model,
                    offsets,
                    read.up_to_date,
                    read.next_offset,
                    transport.appended(),
                )
            });

        prop_assert_eq!(&read_bytes, &model, "the read is the exact concatenation");
        prop_assert!(up_to_date, "the catch-up read reaches the tail");
        prop_assert!(read_offset.is_some(), "the read reports a next offset");
        prop_assert_eq!(
            appends,
            chunks.len(),
            "each chunk is one plain append, with no retry duplicate"
        );
        prop_assert!(
            offsets.windows(2).all(|pair| pair[0] <= pair[1]),
            "the offsets never go backwards"
        );
        let mut previous_with_data: Option<&String> = None;
        for (chunk, offset) in chunks.iter().zip(&offsets) {
            if chunk.is_empty() {
                continue;
            }
            if let Some(previous) = previous_with_data {
                prop_assert!(
                    previous < offset,
                    "an append with data moves the offset forward"
                );
            }
            previous_with_data = Some(offset);
        }
    }
}

/// A read that answers each transient status, then succeeds.
#[cfg(feature = "test-clock")]
#[test]
fn given_each_transient_status_when_a_read_runs_then_the_policy_retries_it() {
    use foundation_context::Clock as _;
    let cases: [(u16, Vec<(&str, &str)>, foundation_context::Duration); 3] = [
        (
            429,
            vec![("retry-after", "2")],
            foundation_context::Duration::from_secs(2),
        ),
        (
            500,
            Vec::new(),
            foundation_context::Duration::from_millis(50),
        ),
        (
            503,
            Vec::new(),
            foundation_context::Duration::from_millis(50),
        ),
    ];

    for (status, headers, expected_wait) in cases {
        let (ctx, timer, clock) = simulated_ctx(31);
        foundation_core::valtron::block_on_future_scoped(async move {
            let transport = MockTransport::with(vec![
                response(status, &headers, b"transient"),
                response(200, &[("content-type", "text/plain")], b"ok"),
            ]);
            let client =
                retrying_client(transport.clone(), ctx, RetryPolicy::default().jitter(0.0));
            let result = client
                .stream(&StreamName::parse("v1/stream/orders").expect("name"))
                .read()
                .send()
                .await
                .unwrap_or_else(|error| panic!("status {status} must retry: {error}"));

            assert_eq!(result.chunks[0].data, b"ok", "status {status} recovers");
            assert_eq!(
                transport.requests.lock().expect("requests").len(),
                2,
                "status {status} retries once"
            );
            assert_eq!(
                timer.delays(),
                vec![expected_wait],
                "status {status} waits as the policy says"
            );
            assert_eq!(
                clock.elapsed(),
                foundation_context::Duration::ZERO,
                "status {status} uses the injected clock, not real time"
            );
        });
    }
}

/// A transport failure is transient, so a read retries it.
#[cfg(feature = "test-clock")]
#[test]
fn given_a_transport_error_when_a_read_runs_then_the_policy_retries_it() {
    use foundation_context::Clock as _;
    let (ctx, timer, clock) = simulated_ctx(37);
    foundation_core::valtron::block_on_future_scoped(async move {
        let transport = MockTransport::with(vec![response(
            200,
            &[("content-type", "text/plain")],
            b"ok",
        )]);
        transport.fail_next(&["connection reset by peer"]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default().jitter(0.0));
        let result = client
            .stream(&StreamName::parse("v1/stream/orders").expect("name"))
            .read()
            .send()
            .await
            .expect("a retried read");

        assert_eq!(result.chunks[0].data, b"ok");
        assert_eq!(transport.requests.lock().expect("requests").len(), 2);
        assert_eq!(
            timer.delays(),
            vec![foundation_context::Duration::from_millis(50)]
        );
        assert_eq!(
            clock.elapsed(),
            foundation_context::Duration::ZERO,
            "the wait came from the injected clock, not real time"
        );
    });
}

/// A timeout that never clears becomes the typed exhaustion error.
#[cfg(feature = "test-clock")]
#[test]
fn given_a_timeout_when_the_policy_exhausts_then_the_error_is_typed() {
    use foundation_context::Clock as _;
    let (ctx, timer, clock) = simulated_ctx(41);
    foundation_core::valtron::block_on_future_scoped(async move {
        let transport = MockTransport::with(Vec::new());
        transport.fail_next(&[
            "request timed out",
            "request timed out",
            "request timed out",
        ]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default().jitter(0.0));
        let error = client
            .stream(&StreamName::parse("v1/stream/orders").expect("name"))
            .read()
            .send()
            .await
            .expect_err("an exhausted timeout");

        match error {
            ClientError::RetryExhausted {
                attempts,
                status,
                body,
                ..
            } => {
                assert_eq!(attempts, 3, "the policy allows three attempts");
                assert_eq!(status, None, "a transport failure has no status");
                assert_eq!(body, "request timed out");
            }
            other => panic!("unexpected error: {other}"),
        }
        assert_eq!(transport.requests.lock().expect("requests").len(), 3);
        assert_eq!(
            timer.delays(),
            vec![
                foundation_context::Duration::from_millis(50),
                foundation_context::Duration::from_millis(100)
            ],
            "two waits between three attempts"
        );
        assert_eq!(
            clock.elapsed(),
            foundation_context::Duration::ZERO,
            "the waits came from the injected clock, not real time"
        );
    });
}

/// A plain append is not safe to retry, so a transient status reaches the caller.
#[cfg(feature = "test-clock")]
#[test]
fn given_a_plain_append_when_a_transient_status_arrives_then_it_is_not_retried() {
    let (ctx, timer, _clock) = simulated_ctx(43);
    foundation_core::valtron::block_on_future_scoped(async move {
        let transport =
            MockTransport::with(vec![response(503, &[], b"down"), response(200, &[], b"ok")]);
        let client = retrying_client(transport.clone(), ctx, RetryPolicy::default());
        let error = client
            .handle("/v1/stream/orders")
            .append(b"x".to_vec(), Some("text/plain"), None, None, false)
            .await
            .expect_err("a plain append is not retried");

        assert!(matches!(
            error,
            ClientError::UnexpectedStatus { status: 503, .. }
        ));
        assert_eq!(
            transport.requests.lock().expect("requests").len(),
            1,
            "the append sends one request only"
        );
        assert_eq!(
            timer.delays(),
            [] as [foundation_context::Duration; 0],
            "a refused append waits for nothing"
        );
    });
}
