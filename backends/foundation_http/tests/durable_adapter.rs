//! Tests for the native Durable Streams HTTP adapter.
//!
//! Each test builds a `DurableService` over the memory VFS and drives the
//! adapter's `dispatch` directly, so the wire rules are checked without a
//! socket.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::sync::Arc;

use foundation_context::Ctx;
use foundation_core::durable::{
    DurableService, DurableStream, MemoryVfs, StoreError, StreamName, StreamOffset,
};
use foundation_http::native::durable::{
    dispatch, error_headers, inline_body_len, live_mode, short_body, sse_control_event,
    sse_control_json, sse_data_event, stream_name_from_path, DurableRequest, DurableResponse,
    LiveMode,
};
use foundation_http::shared::durable::parse::encode_offset;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders, SimpleMethod};

fn service() -> DurableService<DurableStream> {
    DurableService::new(DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host()))
}

fn service_for(stream: DurableStream) -> DurableService<DurableStream> {
    DurableService::new(stream)
}

/// The wire text of a byte position.
fn offset(value: u64) -> String {
    encode_offset(&StreamOffset::from_bytes(value.to_be_bytes().to_vec()))
}

fn headers(pairs: &[(&str, &str)]) -> SimpleHeaders {
    let mut map = SimpleHeaders::new();
    for (key, value) in pairs {
        map.insert(SimpleHeader::custom(*key), vec![(*value).to_string()]);
    }
    map
}

async fn send(
    service: &DurableService<DurableStream>,
    method: SimpleMethod,
    name: &str,
    mut headers: SimpleHeaders,
    queries: Option<BTreeMap<String, String>>,
    body: &str,
) -> Result<DurableResponse, StoreError> {
    if matches!(method, SimpleMethod::POST | SimpleMethod::PUT)
        && !headers.keys().any(|key| key.to_string().eq_ignore_ascii_case("content-type"))
    {
        headers.insert(
            SimpleHeader::custom("content-type"),
            vec!["text/plain".to_string()],
        );
    }
    dispatch(
        service,
        &Ctx::host(),
        DurableRequest {
            method,
            name: StreamName::parse(name).expect("valid stream name"),
            headers,
            queries,
            body: body.as_bytes().to_vec(),
        },
    )
    .await
}

#[test]
fn given_a_new_stream_when_put_runs_then_it_creates_with_a_location() {
    smol::block_on(async {
        let service = service();
        let response = send(
            &service,
            SimpleMethod::PUT,
            "events",
            headers(&[("content-type", "application/json")]),
            None,
            "",
        )
        .await
        .expect("create");

        assert_eq!(response.status, 201);
        assert_eq!(response.header("Location"), Some("/events"));
        assert_eq!(
            response.header("Stream-Next-Offset"),
            Some(offset(0).as_str())
        );
        assert_eq!(response.header("Content-Type"), Some("application/json"));
    });
}

#[test]
fn given_an_existing_stream_when_put_runs_again_with_the_same_spec_then_it_is_not_created_again() {
    smol::block_on(async {
        let service = service();
        let first = send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "").await;
        assert_eq!(first.expect("create").status, 201);

        let second = send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "").await;
        assert_eq!(second.expect("existing").status, 200);
    });
}

#[test]
fn given_a_stream_when_post_appends_then_get_returns_the_body_and_is_up_to_date() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "").await
            .expect("create");

        let append = send(&service, SimpleMethod::POST, "events", headers(&[]), None, "hello")
            .await
            .expect("append");
        assert_eq!(append.status, 204);
        let offset = append
            .header("Stream-Next-Offset")
            .expect("next offset")
            .to_string();

        let read = send(&service, SimpleMethod::GET, "events", headers(&[]), None, "")
            .await
            .expect("read");
        assert_eq!(read.status, 200);
        assert_eq!(read.body, b"hello");
        assert_eq!(read.header("Stream-Up-To-Date"), Some("true"));
        assert_eq!(read.header("Stream-Next-Offset"), Some(offset.as_str()));
        assert_eq!(read.header("Stream-Closed"), None);
    });
}

#[test]
fn given_a_json_stream_when_get_runs_then_the_body_is_a_json_array() {
    smol::block_on(async {
        let service = service();
        send(
            &service,
            SimpleMethod::PUT,
            "events",
            headers(&[("content-type", "application/json")]),
            None,
            "",
        )
        .await
        .expect("create");
        send(
            &service,
            SimpleMethod::POST,
            "events",
            headers(&[("content-type", "application/json")]),
            None,
            r#"{"a":1}"#,
        )
        .await
        .expect("append one");
        send(
            &service,
            SimpleMethod::POST,
            "events",
            headers(&[("content-type", "application/json")]),
            None,
            r#"{"a":2}"#,
        )
        .await
        .expect("append two");

        let read = send(&service, SimpleMethod::GET, "events", headers(&[]), None, "")
            .await
            .expect("read");
        assert_eq!(read.body, br#"[{"a":1},{"a":2}]"#);
    });
}

#[test]
fn given_a_stream_when_get_uses_offset_minus_one_then_it_reads_from_the_start() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "abc").await
            .expect("create");

        let mut queries = BTreeMap::new();
        queries.insert("offset".to_string(), "-1".to_string());
        let read = send(
            &service,
            SimpleMethod::GET,
            "events",
            headers(&[]),
            Some(queries),
            "",
        )
        .await
        .expect("read");
        assert_eq!(read.body, b"abc");
    });
}

#[test]
fn given_a_stream_when_a_second_read_uses_the_next_offset_then_it_gets_no_chunks() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "abc").await
            .expect("create");

        let first = send(&service, SimpleMethod::GET, "events", headers(&[]), None, "")
            .await
            .expect("read");
        let next = first.header("Stream-Next-Offset").expect("offset").to_string();

        let mut queries = BTreeMap::new();
        queries.insert("offset".to_string(), next);
        let second = send(
            &service,
            SimpleMethod::GET,
            "events",
            headers(&[]),
            Some(queries),
            "",
        )
        .await
        .expect("read");
        assert_eq!(second.body, b"");
        assert_eq!(second.header("Stream-Up-To-Date"), Some("true"));
    });
}

#[test]
fn given_a_stream_when_head_runs_then_it_reports_the_tail_without_a_body() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "abc").await
            .expect("create");

        let response = send(&service, SimpleMethod::HEAD, "events", headers(&[]), None, "")
            .await
            .expect("head");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, Vec::<u8>::new());
        assert_eq!(
            response.header("Stream-Next-Offset"),
            Some(offset(3).as_str())
        );
        assert_eq!(response.header("Cache-Control"), Some("no-store"));
        assert_eq!(response.header("ETag"), Some("\"events:0:3\""));
    });
}

#[test]
fn given_a_stream_when_a_post_closes_it_then_the_closed_header_is_set_on_read() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "").await
            .expect("create");

        let close = send(
            &service,
            SimpleMethod::POST,
            "events",
            headers(&[("stream-closed", "true")]),
            None,
            "bye",
        )
        .await
        .expect("close");
        assert_eq!(close.header("Stream-Closed"), Some("true"));

        let read = send(&service, SimpleMethod::GET, "events", headers(&[]), None, "")
            .await
            .expect("read");
        assert_eq!(read.body, b"bye");
        assert_eq!(read.header("Stream-Closed"), Some("true"));
    });
}

#[test]
fn given_a_producer_when_the_same_seq_arrives_twice_then_only_one_chunk_is_stored() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "").await
            .expect("create");

        let producer = headers(&[
            ("producer-id", "p1"),
            ("producer-epoch", "1"),
            ("producer-seq", "0"),
        ]);
        let first = send(&service, SimpleMethod::POST, "events", producer.clone(), None, "x")
            .await
            .expect("first");
        assert_eq!(first.status, 200);
        let second = send(&service, SimpleMethod::POST, "events", producer, None, "x")
            .await
            .expect("duplicate");
        assert_eq!(second.status, 204);

        let read = send(&service, SimpleMethod::GET, "events", headers(&[]), None, "")
            .await
            .expect("read");
        assert_eq!(read.body, b"x");
    });
}

#[test]
fn given_a_matching_etag_when_get_runs_then_it_returns_304() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "abc").await
            .expect("create");

        let first = send(&service, SimpleMethod::GET, "events", headers(&[]), None, "")
            .await
            .expect("read");
        let tag = first.header("ETag").expect("etag").to_string();

        let second = send(
            &service,
            SimpleMethod::GET,
            "events",
            headers(&[("if-none-match", &tag)]),
            None,
            "",
        )
        .await
        .expect("conditional read");
        assert_eq!(second.status, 304);
        assert_eq!(second.body, Vec::<u8>::new());
    });
}

#[test]
fn given_a_missing_stream_when_get_runs_then_it_reports_not_found() {
    smol::block_on(async {
        let service = service();
        let error = send(&service, SimpleMethod::GET, "absent", headers(&[]), None, "")
            .await
            .expect_err("missing");
        assert!(matches!(error, StoreError::NotFound));
    });
}

#[test]
fn given_a_stream_when_delete_runs_then_get_reports_gone() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "").await
            .expect("create");

        let deleted = send(&service, SimpleMethod::DELETE, "events", headers(&[]), None, "")
            .await
            .expect("delete");
        assert_eq!(deleted.status, 204);

        let error = send(&service, SimpleMethod::GET, "events", headers(&[]), None, "")
            .await
            .expect_err("gone");
        assert!(matches!(error, StoreError::Gone | StoreError::NotFound));
    });
}

#[test]
fn given_an_unsupported_method_when_dispatch_runs_then_it_answers_405() {
    smol::block_on(async {
        let service = service();
        let response = send(&service, SimpleMethod::PATCH, "events", headers(&[]), None, "")
            .await
            .expect("405");
        assert_eq!(response.status, 405);
    });
}

#[test]
fn given_a_path_with_a_query_when_it_is_parsed_then_the_query_is_dropped() {
    let name = stream_name_from_path("/a/b?offset=now").expect("name");
    assert_eq!(name.as_str(), "a/b");
}

#[test]
fn given_a_partial_producer_header_set_when_post_runs_then_it_is_a_bad_request() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "").await
            .expect("create");

        let error = send(
            &service,
            SimpleMethod::POST,
            "events",
            headers(&[("producer-id", "p1")]),
            None,
            "x",
        )
        .await
        .expect_err("partial");
        assert!(matches!(error, StoreError::BadRequest(_)));
    });
}

#[test]
fn given_live_long_poll_values_when_parsed_then_only_known_modes_are_accepted() {
    assert_eq!(live_mode(None).expect("catch-up"), LiveMode::CatchUp);

    let mut queries = BTreeMap::new();
    queries.insert("live".to_string(), "long-poll".to_string());
    assert_eq!(
        live_mode(Some(&queries)).expect("long-poll"),
        LiveMode::LongPoll
    );

    queries.insert("live".to_string(), "sse".to_string());
    assert_eq!(live_mode(Some(&queries)).expect("sse"), LiveMode::Sse);

    queries.insert("live".to_string(), "stream".to_string());
    assert!(live_mode(Some(&queries)).is_err());
}

#[test]
fn given_a_stream_with_data_when_long_poll_runs_then_it_returns_the_data_at_once() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "abc").await
            .expect("create");

        let mut queries = BTreeMap::new();
        queries.insert("offset".to_string(), "-1".to_string());
        queries.insert("live".to_string(), "long-poll".to_string());
        let response = send(
            &service,
            SimpleMethod::GET,
            "events",
            headers(&[]),
            Some(queries),
            "",
        )
        .await
        .expect("long-poll");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"abc");
        assert!(response.header("Stream-Cursor").is_some());
    });
}

#[test]
fn given_an_open_stream_at_the_tail_when_an_append_arrives_then_long_poll_returns_it() {
    smol::block_on(async {
        let stream = DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host());
        let service = DurableService::new(stream.clone());
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "")
            .await
            .expect("create");
        let offset = send(
            &service,
            SimpleMethod::GET,
            "events",
            headers(&[]),
            None,
            "",
        )
        .await
        .expect("read")
        .header("Stream-Next-Offset")
        .expect("offset")
        .to_string();

        let appender = service_for(stream);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            smol::block_on(async {
                send(
                    &appender,
                    SimpleMethod::POST,
                    "events",
                    headers(&[]),
                    None,
                    "late",
                )
                .await
                .expect("append");
            });
        });

        let mut queries = BTreeMap::new();
        queries.insert("offset".to_string(), offset);
        queries.insert("live".to_string(), "long-poll".to_string());
        let response = send(
            &service,
            SimpleMethod::GET,
            "events",
            headers(&[]),
            Some(queries),
            "",
        )
        .await
        .expect("long-poll");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"late");
    });
}

#[test]
fn given_a_closed_stream_at_the_tail_when_long_poll_runs_then_it_returns_204_closed() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "abc").await
            .expect("create");
        send(
            &service,
            SimpleMethod::POST,
            "events",
            headers(&[("stream-closed", "true")]),
            None,
            "",
        )
        .await
        .expect("close");

        let mut queries = BTreeMap::new();
        queries.insert("offset".to_string(), "-1".to_string());
        queries.insert("live".to_string(), "long-poll".to_string());
        let response = send(
            &service,
            SimpleMethod::GET,
            "events",
            headers(&[]),
            Some(queries),
            "",
        )
        .await
        .expect("long-poll");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"abc");

        let next = response
            .header("Stream-Next-Offset")
            .expect("offset")
            .to_string();
        let mut queries = BTreeMap::new();
        queries.insert("offset".to_string(), next);
        queries.insert("live".to_string(), "long-poll".to_string());
        let eof = send(
            &service,
            SimpleMethod::GET,
            "events",
            headers(&[]),
            Some(queries),
            "",
        )
        .await
        .expect("long-poll eof");
        assert_eq!(eof.status, 204);
        assert_eq!(eof.header("Stream-Closed"), Some("true"));
        assert_eq!(eof.header("Stream-Up-To-Date"), Some("true"));
    });
}

#[test]
fn given_a_long_poll_without_an_offset_when_dispatch_runs_then_it_is_a_bad_request() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "events", headers(&[]), None, "").await
            .expect("create");

        let mut queries = BTreeMap::new();
        queries.insert("live".to_string(), "long-poll".to_string());
        let error = send(
            &service,
            SimpleMethod::GET,
            "events",
            headers(&[]),
            Some(queries),
            "",
        )
        .await
        .expect_err("bad request");
        assert!(matches!(error, StoreError::BadRequest(_)));
    });
}

#[test]
fn given_a_text_payload_with_newlines_when_it_becomes_a_data_event_then_each_line_has_a_prefix() {
    let event = sse_data_event(b"a\nb\n");
    assert_eq!(
        String::from_utf8(event).expect("utf-8"),
        "event: data\ndata:a\ndata:b\n\n"
    );
}

#[test]
fn given_a_control_step_when_it_is_rendered_then_it_carries_the_offset_and_cursor() {
    let next = StreamOffset::from_bytes(4_u64.to_be_bytes().to_vec());
    let json = sse_control_json(&next, Some(7), true, false);
    assert!(json.contains(&format!("\"streamNextOffset\":\"{}\"", encode_offset(&next))));
    assert!(json.contains("\"streamCursor\":7"));
    assert!(json.contains("\"upToDate\":true"));
    assert!(!json.contains("streamClosed"));

    let closed = sse_control_json(&next, None, true, true);
    assert!(closed.contains("\"streamClosed\":true"));
    assert!(!closed.contains("streamCursor"));

    let event = String::from_utf8(sse_control_event(&json)).expect("utf-8");
    assert_eq!(event, format!("event: control\ndata:{json}\n\n"));
}

#[test]
fn given_a_declared_length_when_the_body_is_short_then_the_adapter_closes_the_connection() {
    assert!(short_body(Some(10), 3));
    assert!(short_body(Some(3), 10));
    assert!(!short_body(Some(3), 3));
    assert!(!short_body(None, 0));
}

#[test]
fn given_a_rate_limit_error_when_the_headers_are_built_then_retry_after_is_present() {
    let headers = error_headers(&StoreError::RateLimited {
        retry_after: Some(std::time::Duration::from_secs(5)),
    });
    assert!(headers
        .iter()
        .any(|(key, value)| key.to_string().eq_ignore_ascii_case("retry-after") && value == "5"));

    let none = error_headers(&StoreError::RateLimited { retry_after: None });
    assert!(none.is_empty());
}

#[test]
fn given_an_inline_body_when_its_length_is_read_then_streamed_bodies_report_none() {
    use foundation_netio::shared::http::SendSafeBody;
    assert_eq!(inline_body_len(&SendSafeBody::Bytes(vec![1, 2, 3])), Some(3));
    assert_eq!(inline_body_len(&SendSafeBody::Text("ab".to_string())), Some(2));
    assert_eq!(inline_body_len(&SendSafeBody::None), None);
    assert_eq!(inline_body_len(&SendSafeBody::Stream(None)), None);
}

#[test]
fn given_a_closed_stream_when_the_old_etag_is_sent_then_the_response_is_not_304() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "etag-close", headers(&[]), None, "abc")
            .await
            .expect("create");

        let first = send(&service, SimpleMethod::GET, "etag-close", headers(&[]), None, "")
            .await
            .expect("read");
        let tag = first.header("ETag").expect("etag").to_string();

        send(
            &service,
            SimpleMethod::POST,
            "etag-close",
            headers(&[("stream-closed", "true")]),
            None,
            "",
        )
        .await
        .expect("close");

        // The ETag changes on closure, so a stale tag cannot hide the EOF.
        let second = send(
            &service,
            SimpleMethod::GET,
            "etag-close",
            headers(&[("if-none-match", &tag)]),
            None,
            "",
        )
        .await
        .expect("conditional read");
        assert_eq!(second.status, 200);
        assert_eq!(second.header("Stream-Closed"), Some("true"));
    });
}

#[test]
fn given_a_preflight_when_it_runs_then_it_allows_the_protocol_headers() {
    smol::block_on(async {
        let service = service();
        let response = send(&service, SimpleMethod::OPTIONS, "any", headers(&[]), None, "")
            .await
            .expect("preflight");

        assert_eq!(response.status, 204);
        assert_eq!(
            response.header("access-control-allow-headers"),
            Some("*")
        );
        assert!(response
            .header("access-control-expose-headers")
            .is_some_and(|value| value.contains("Stream-Next-Offset")));
    });
}

#[test]
fn given_an_absolute_expiry_when_head_runs_then_the_timestamp_is_returned() {
    smol::block_on(async {
        let service = service();
        let expires_at = "2030-01-01T00:00:00Z";
        send(
            &service,
            SimpleMethod::PUT,
            "expires-head",
            headers(&[("stream-expires-at", expires_at)]),
            None,
            "",
        )
        .await
        .expect("create");

        let head = send(&service, SimpleMethod::HEAD, "expires-head", headers(&[]), None, "")
            .await
            .expect("head");
        assert_eq!(head.header("Stream-Expires-At"), Some(expires_at));
    });
}

#[test]
fn given_new_data_when_a_read_runs_then_the_etag_changes() {
    smol::block_on(async {
        let service = service();
        send(&service, SimpleMethod::PUT, "etag-data", headers(&[]), None, "abc")
            .await
            .expect("create");
        let first = send(&service, SimpleMethod::GET, "etag-data", headers(&[]), None, "")
            .await
            .expect("read");
        let first_tag = first.header("ETag").expect("etag").to_string();

        send(&service, SimpleMethod::POST, "etag-data", headers(&[]), None, "def")
            .await
            .expect("append");
        let second = send(&service, SimpleMethod::GET, "etag-data", headers(&[]), None, "")
            .await
            .expect("read");
        let second_tag = second.header("ETag").expect("etag").to_string();

        assert_ne!(first_tag, second_tag);
        assert_eq!(second.body, b"abcdef");
    });
}

#[test]
fn given_producer_errors_when_the_headers_are_built_then_the_contract_fields_are_present() {
    let gap = error_headers(&StoreError::SequenceGap {
        expected: 1,
        received: 2,
    });
    assert!(gap
        .iter()
        .any(|(key, value)| key.to_string().eq_ignore_ascii_case("producer-expected-seq")
            && value == "1"));
    assert!(gap
        .iter()
        .any(|(key, value)| key.to_string().eq_ignore_ascii_case("producer-received-seq")
            && value == "2"));

    let fenced = error_headers(&StoreError::Fenced { current_epoch: 5 });
    assert!(fenced
        .iter()
        .any(|(key, value)| key.to_string().eq_ignore_ascii_case("producer-epoch") && value == "5"));
}
