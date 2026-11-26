//! Tests for the native Durable Streams HTTP adapter.
//!
//! Each test builds a `DurableService` over the memory VFS and drives the
//! adapter's `dispatch` directly, so the wire rules are checked without a
//! socket.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::sync::Arc;

use foundation_core::durable::{
    DurableService, DurableStream, MemoryVfs, StoreError, StreamName, StreamOffset,
};
use foundation_http::native::durable::{
    dispatch, stream_name_from_path, DurableRequest, DurableResponse,
};
use foundation_http::shared::durable::parse::encode_offset;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders, SimpleMethod};

fn service() -> DurableService<DurableStream> {
    DurableService::new(DurableStream::new(Arc::new(MemoryVfs::new())))
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
    headers: SimpleHeaders,
    queries: Option<BTreeMap<String, String>>,
    body: &str,
) -> Result<DurableResponse, StoreError> {
    dispatch(
        service,
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
        assert!(response.body.is_empty());
        assert_eq!(
            response.header("Stream-Next-Offset"),
            Some(offset(3).as_str())
        );
        assert_eq!(response.header("Cache-Control"), Some("no-store"));
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
        assert!(second.body.is_empty());
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
