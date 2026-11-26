//! Given/When/Then scenarios for the Durable Streams client core.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use foundation_core::durable::StreamName;
use foundation_core::durable::client::{
    ClientConfig, ClientError, ClientFuture, ClientRequest, ClientResponse, ClientSseStream,
    ClientTransport, DurableClient, ReadMode, SseEvent, TransportError,
};

#[derive(Default)]
struct MockTransport {
    responses: Mutex<VecDeque<ClientResponse>>,
    requests: Mutex<Vec<ClientRequest>>,
    events: Mutex<Vec<Result<SseEvent, TransportError>>>,
}

impl MockTransport {
    fn with(responses: Vec<ClientResponse>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(Vec::new()),
            events: Mutex::new(Vec::new()),
        })
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

    fn open_sse(&self, request: ClientRequest) -> ClientFuture<'_, ClientSseStream> {
        self.requests.lock().expect("requests").push(request);
        let events = self.events.lock().expect("events").drain(..).collect::<Vec<_>>();
        Box::pin(async move {
            let stream = futures_lite::stream::iter(events);
            Ok(Box::pin(stream) as ClientSseStream)
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
            .create(Some("application/json"), Some(60), None, false, b"abc".to_vec())
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
        let result = handle(transport)
            .read()
            .send()
            .await
            .expect("read");

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
        assert!(result.chunks.is_empty());
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

fn sse(event_type: &str, data: &str) -> Result<SseEvent, TransportError> {
    Ok(SseEvent {
        event_type: Some(event_type.to_string()),
        data: data.to_string(),
    })
}

#[test]
fn given_a_sse_data_and_control_event_when_a_read_runs_then_it_reports_the_chunk_and_offset() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(200, &[], b"")]);
        transport.events.lock().expect("events").extend([
            sse("data", "hello"),
            sse(
                "control",
                r#"{"streamNextOffset":"00ff","streamCursor":7,"upToDate":true}"#,
            ),
        ]);
        let result = handle(transport).read().mode(ReadMode::Sse).send().await.expect("sse");

        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].data, b"hello");
        assert_eq!(result.next_offset.as_deref(), Some("00ff"));
        assert_eq!(result.cursor.as_deref(), Some("7"));
        assert!(result.up_to_date);
    });
}

#[test]
fn given_a_base64_sse_payload_when_a_binary_read_runs_then_it_decodes_the_bytes() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(200, &[], b"")]);
        transport.events.lock().expect("events").extend([
            sse("data", "SGVsbG8="),
            sse("control", r#"{"streamNextOffset":"00ff","streamClosed":true}"#),
        ]);
        let result = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .sse_binary(true)
            .send()
            .await
            .expect("sse");

        assert_eq!(result.chunks[0].data, b"Hello");
        assert!(result.closed);
    });
}

#[test]
fn given_a_control_event_with_no_type_when_a_read_runs_then_it_still_ends_the_read() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(200, &[], b"")]);
        transport.events.lock().expect("events").push(Ok(SseEvent {
            event_type: None,
            data: r#"{"streamNextOffset":"00aa","upToDate":true}"#.to_string(),
        }));
        let result = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .send()
            .await
            .expect("sse");

        assert!(result.chunks.is_empty());
        assert_eq!(result.next_offset.as_deref(), Some("00aa"));
    });
}

#[test]
fn given_an_empty_data_event_when_a_read_runs_then_it_adds_no_chunk() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(200, &[], b"")]);
        transport.events.lock().expect("events").extend([
            sse("data", ""),
            sse("control", r#"{"streamNextOffset":"00bb","upToDate":true}"#),
        ]);
        let result = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .send()
            .await
            .expect("sse");

        assert!(result.chunks.is_empty());
    });
}

#[test]
fn given_a_missing_stream_when_an_sse_read_runs_then_the_probe_answers_not_found() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(404, &[], b"")]);
        let error = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .send()
            .await
            .expect_err("missing");
        assert!(matches!(error, ClientError::NotFound { .. }));
    });
}

#[test]
fn given_an_sse_encoding_header_when_a_text_read_runs_then_the_data_is_decoded() {
    foundation_core::valtron::block_on_future_scoped(async {
        let transport = MockTransport::with(vec![response(
            200,
            &[("stream-sse-data-encoding", "base64")],
            b"",
        )]);
        transport.events.lock().expect("events").extend([
            sse("data", "SGVsbG8="),
            sse("control", r#"{"streamNextOffset":"00cc"}"#),
        ]);
        let result = handle(transport)
            .read()
            .mode(ReadMode::Sse)
            .send()
            .await
            .expect("sse");

        assert_eq!(result.chunks[0].data, b"Hello");
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
        assert!(result.chunks.is_empty());
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

        assert!(result.chunks.is_empty());
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
    use foundation_core::durable::client::valtron::ClientReader;
    use foundation_core::valtron::{Stream, StreamSpread};

    let transport = MockTransport::with(vec![
        response(200, &[("stream-next-offset", "01")], b"ab"),
        response(
            200,
            &[("stream-next-offset", "02"), ("stream-up-to-date", "true")],
            b"cd",
        ),
    ]);
    let mut reader = ClientReader::new(handle(transport), foundation_context::Ctx::host());

    let mut bytes = Vec::new();
    while let Some(item) = reader.next() {
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
