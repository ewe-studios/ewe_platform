//! Connect protocol tests (spec-41 F19 / Decision 05): content-type mechanics,
//! unary error JSON, GET query encoding + fallback precheck, client headers, and
//! a full streaming round-trip through both protocol reader/writer stacks proving
//! `Frame::EndStream` normalization.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::valtron::Pipe;
use foundation_netio::simple_http::shared::{
    SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};

use foundation_connectrpc::context::{CancelSignal, Spec, StreamType};
use foundation_connectrpc::protocol::connect::{
    constants, encode_get_query, get_url_exceeds, streaming_content_type, unary_content_type,
    unary_error_body, unary_error_status, ConnectClient, ConnectHandler,
};
use foundation_connectrpc::protocol::{
    canonicalize_content_type, parse_connect_content_type, ProtocolClient, ProtocolHandler,
};
use foundation_connectrpc::transport::TransportStream;
use foundation_connectrpc::{CompressionRegistry, ConnectError};

// ── content-type mechanics ────────────────────────────────────────────────────

#[test]
fn content_type_canonicalization_and_parse() {
    assert_eq!(
        canonicalize_content_type("application/JSON; charset=utf-8"),
        "application/json"
    );
    assert_eq!(
        parse_connect_content_type("application/proto"),
        Some(("proto".to_string(), false))
    );
    assert_eq!(
        parse_connect_content_type("application/connect+json; charset=utf-8"),
        Some(("json".to_string(), true))
    );
    assert_eq!(parse_connect_content_type("text/plain"), None);
    assert_eq!(unary_content_type("proto"), "application/proto");
    assert_eq!(streaming_content_type("json"), "application/connect+json");
}

// ── unary error wire ──────────────────────────────────────────────────────────

#[test]
fn unary_error_status_and_body_match_spec() {
    let err: foundation_errstacks::ErrorTrace<ConnectError> =
        ConnectError::unavailable("overloaded").into();
    assert_eq!(unary_error_status(&err), 503);
    let body = unary_error_body(&err).unwrap();
    assert_eq!(&body, br#"{"code":"unavailable","message":"overloaded"}"#);

    let nf: foundation_errstacks::ErrorTrace<ConnectError> =
        ConnectError::not_found("nope").into();
    assert_eq!(unary_error_status(&nf), 404);
}

// ── GET query encoding + fallback precheck ────────────────────────────────────

#[test]
fn get_query_text_codec_is_percent_encoded() {
    // JSON (text) codec, no compression → percent-encoded message, no base64 flag.
    let q = encode_get_query("json", br#"{"a":1}"#, false, None).unwrap();
    assert_eq!(
        q,
        "connect=v1&encoding=json&message=%7B%22a%22%3A1%7D"
    );
}

#[test]
fn get_query_binary_codec_is_base64url() {
    // proto (binary) codec → base64url message + base64=1.
    let q = encode_get_query("proto", &[0xFF, 0x00, 0x10], true, None).unwrap();
    assert_eq!(q, "connect=v1&encoding=proto&message=_wAQ&base64=1");
}

#[test]
fn get_url_fallback_precheck() {
    assert!(get_url_exceeds(9000, 8192));
    assert!(!get_url_exceeds(100, 8192));
    assert!(!get_url_exceeds(9000, 0)); // 0 = unlimited
}

// ── client request headers ────────────────────────────────────────────────────

#[test]
fn client_headers_unary_vs_streaming() {
    let client = ConnectClient;

    let mut unary = SimpleHeaders::new();
    client.write_request_headers(StreamType::Unary, &mut unary, "proto", Some("gzip"));
    assert_eq!(
        unary.get(&SimpleHeader::CONTENT_TYPE).unwrap(),
        &vec!["application/proto".to_string()]
    );
    assert_eq!(
        unary
            .get(&SimpleHeader::from(
                constants::HEADER_PROTOCOL_VERSION.to_string()
            ))
            .unwrap(),
        &vec!["1".to_string()]
    );
    // Unary uses standard Content-Encoding.
    assert!(unary
        .get(&SimpleHeader::from("content-encoding".to_string()))
        .is_some());

    let mut streaming = SimpleHeaders::new();
    client.write_request_headers(StreamType::ServerStream, &mut streaming, "json", Some("gzip"));
    assert_eq!(
        streaming.get(&SimpleHeader::CONTENT_TYPE).unwrap(),
        &vec!["application/connect+json".to_string()]
    );
    // Streaming uses Connect-Content-Encoding.
    assert!(streaming
        .get(&SimpleHeader::from(
            constants::HEADER_STREAMING_CONTENT_ENCODING.to_string()
        ))
        .is_some());
}

// ── streaming round-trip (both reader/writer stacks) ──────────────────────────

type BoxFut = Pin<Box<dyn Future<Output = ()>>>;

fn run_all(mut futs: Vec<BoxFut>) {
    struct W(std::thread::Thread);
    impl Wake for W {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(W(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !futs.is_empty() {
        let mut i = 0;
        while i < futs.len() {
            match futs[i].as_mut().poll(&mut cx) {
                Poll::Ready(()) => {
                    let _ = futs.swap_remove(i);
                }
                Poll::Pending => i += 1,
            }
        }
        if futs.is_empty() {
            break;
        }
        assert!(Instant::now() < deadline, "streaming round-trip deadlocked");
        std::thread::park_timeout(Duration::from_millis(5));
    }
}

fn streaming_request() -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_plain_url("http://t/svc.Svc/Echo")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/connect+proto")
        .build()
        .expect("request")
}

#[test]
fn streaming_roundtrip_normalizes_endstream() {
    let registry = CompressionRegistry::new();

    // The "wire" byte pipes between client and server.
    let (req_tx, req_rx) = Pipe::<Bytes>::with_depth(8);
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(8);

    let server = ConnectHandler
        .new_conn(
            &streaming_request(),
            Spec {
                stream_type: StreamType::BidiStream,
                procedure: "svc.Svc/Echo".to_string(),
                is_client: false,
                idempotency: foundation_connectrpc::IdempotencyLevel::Unknown,
            },
            req_rx,
            resp_tx,
            &registry,
        )
        .expect("server exchange");
    // new_conn ignores the response head here; provide an open (empty) head
    // receiver whose sender stays parked for the test's lifetime.
    let (_client_head_tx, client_head_rx): (
        foundation_core::valtron::PipeSender<(
            foundation_netio::simple_http::shared::Status,
            foundation_netio::simple_http::shared::SimpleHeaders,
        )>,
        foundation_connectrpc::transport::HeadSource,
    ) = foundation_core::valtron::Pipe::with_depth(1);
    let client = ConnectClient
        .new_conn(
            &Spec {
                stream_type: StreamType::BidiStream,
                procedure: "svc.Svc/Echo".to_string(),
                is_client: true,
                idempotency: foundation_connectrpc::IdempotencyLevel::Unknown,
            },
            SimpleHeaders::new(),
            TransportStream {
                send_body: req_tx,
                // F45 Part D: TransportStream now speaks FutureStream; bridge the
                // pipe-fed test harness through the pipe→FutureStream adapters.
                head: foundation_connectrpc::transport::head_stream_from_pipe(client_head_rx),
                recv_body: foundation_connectrpc::transport::body_stream_from_pipe(resp_rx),
            },
            CancelSignal::new(),
        )
        .expect("client exchange");

    let (mut server_rx, server_tx) = server.conn.split();
    let (mut client_tx, mut client_rx) = client.conn.split();

    let received: Arc<Mutex<Vec<Bytes>>> = Arc::new(Mutex::new(Vec::new()));
    let got_clean_eos = Arc::new(Mutex::new(false));

    // Client sends two messages then half-closes.
    let cs: BoxFut = Box::pin(async move {
        client_tx.send(Bytes::from_static(b"m1")).await.unwrap();
        client_tx.send(Bytes::from_static(b"m2")).await.unwrap();
        client_tx.close_send().await.unwrap();
    });

    // Server echoes each message, then closes with trailers (EndStream).
    let mut server_sender = server_tx;
    let srv: BoxFut = Box::pin(async move {
        while let Some(bytes) = server_rx.receive().await.unwrap() {
            server_sender.send(bytes).await.unwrap();
        }
        let mut trailers = SimpleHeaders::new();
        trailers.insert(SimpleHeader::from("x-count".to_string()), vec!["2".to_string()]);
        server_sender.close(None, trailers).await.unwrap();
    });

    // Client receives echoes; clean EOS (Ok(None)) is the normalized EndStream.
    let recv = Arc::clone(&received);
    let eos = Arc::clone(&got_clean_eos);
    let cr: BoxFut = Box::pin(async move {
        loop {
            match client_rx.receive().await.unwrap() {
                Some(b) => recv.lock().unwrap().push(b),
                None => {
                    *eos.lock().unwrap() = true;
                    break;
                }
            }
        }
    });

    let s_read = server.reader_task;
    let s_write = server.writer_task;
    let c_read = client.reader_task;
    let c_write = client.writer_task;
    run_all(vec![
        cs,
        srv,
        cr,
        Box::pin(async move { let _ = s_read.await; }),
        Box::pin(async move { let _ = s_write.await; }),
        Box::pin(async move { let _ = c_read.await; }),
        Box::pin(async move { let _ = c_write.await; }),
    ]);

    let msgs = received.lock().unwrap();
    assert_eq!(
        &*msgs,
        &[Bytes::from_static(b"m1"), Bytes::from_static(b"m2")]
    );
    assert!(*got_clean_eos.lock().unwrap(), "EndStream normalized to Ok(None)");
}
