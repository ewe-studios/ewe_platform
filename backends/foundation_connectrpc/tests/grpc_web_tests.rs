//! gRPC-Web tests (spec-41 F20 / Decision 05): status trailer build/parse with
//! the details-bin preference (P9), streaming base64 (fragmented boundaries), and
//! binary + text streaming round-trips through both reader/writer stacks.

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

use foundation_connectrpc::context::{CancelSignal, IdempotencyLevel, Spec, StreamType};
use foundation_connectrpc::protocol::grpc_web::{
    build_status_trailers, constants, content_type, parse_status_trailers, Base64StreamDecoder,
    Base64StreamEncoder, GrpcWebClient, GrpcWebHandler,
};
use foundation_connectrpc::protocol::{ProtocolClient, ProtocolHandler};
use foundation_connectrpc::transport::TransportStream;
use foundation_connectrpc::{Code, CompressionRegistry, ConnectError};

// ── status trailers ───────────────────────────────────────────────────────────

#[test]
fn status_trailers_success_and_error() {
    let ok = build_status_trailers(None, &SimpleHeaders::new());
    assert_eq!(
        ok.get(&SimpleHeader::from(constants::TRAILER_STATUS.to_string()))
            .unwrap(),
        &vec!["0".to_string()]
    );
    assert!(parse_status_trailers(&ok).is_none());

    let err: foundation_errstacks::ErrorTrace<ConnectError> =
        ConnectError::permission_denied("no access").into();
    let trailers = build_status_trailers(Some(&err), &SimpleHeaders::new());
    // grpc-status = 7 (PermissionDenied).
    assert_eq!(
        trailers
            .get(&SimpleHeader::from(constants::TRAILER_STATUS.to_string()))
            .unwrap(),
        &vec!["7".to_string()]
    );
    let parsed = parse_status_trailers(&trailers).expect("error");
    assert_eq!(parsed.current_context().code(), Code::PermissionDenied);
    assert_eq!(parsed.current_context().message(), "no access");
}

#[test]
fn status_details_bin_preferred_over_status_message() {
    // Craft trailers where grpc-status says 3 but the details-bin says 5.
    let mut trailers = build_status_trailers(
        Some(&ConnectError::invalid_argument("plain").into()),
        &SimpleHeaders::new(),
    );
    // Override grpc-status/message to disagree with details-bin.
    trailers.insert(
        SimpleHeader::from(constants::TRAILER_STATUS.to_string()),
        vec!["3".to_string()], // InvalidArgument
    );
    // details-bin still encodes NotFound(5) "detailed".
    let details = foundation_connectrpc::protocol::grpc_web::encode_status_proto(5, "detailed");
    use base64::Engine as _;
    trailers.insert(
        SimpleHeader::from(constants::TRAILER_STATUS_DETAILS.to_string()),
        vec![base64::engine::general_purpose::STANDARD.encode(details)],
    );

    let parsed = parse_status_trailers(&trailers).expect("error");
    // P9: details-bin wins.
    assert_eq!(parsed.current_context().code(), Code::NotFound);
    assert_eq!(parsed.current_context().message(), "detailed");
}

#[test]
fn grpc_message_percent_encoding_roundtrip() {
    use foundation_connectrpc::protocol::grpc_web::{percent_decode_message, percent_encode_message};
    let msg = "needs %encoding: café\n";
    let enc = percent_encode_message(msg);
    assert!(!enc.contains('é'));
    assert_eq!(percent_decode_message(&enc), msg);
}

// ── streaming base64 (text mode) ──────────────────────────────────────────────

#[test]
fn streaming_base64_handles_fragmented_boundaries() {
    let data: Vec<u8> = (0u8..=200).collect();

    // Encode the whole thing feeding 1 byte at a time.
    let mut enc = Base64StreamEncoder::default();
    let mut encoded = Vec::new();
    for b in &data {
        encoded.extend(enc.feed(&[*b]));
    }
    encoded.extend(enc.finish());

    // The stream equals a one-shot standard base64 encode.
    use base64::Engine as _;
    assert_eq!(
        String::from_utf8(encoded.clone()).unwrap(),
        base64::engine::general_purpose::STANDARD.encode(&data)
    );

    // Decode feeding 3 base64 chars at a time (misaligned to 4-char groups).
    let mut dec = Base64StreamDecoder::default();
    let mut decoded = Vec::new();
    for chunk in encoded.chunks(3) {
        decoded.extend(dec.feed(chunk).unwrap());
    }
    assert_eq!(decoded, data);
}

// ── streaming round-trips ─────────────────────────────────────────────────────

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
        assert!(Instant::now() < deadline, "grpc-web round-trip deadlocked");
        std::thread::park_timeout(Duration::from_millis(5));
    }
}

fn request(text: bool) -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_plain_url("http://t/svc.Svc/Echo")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_TYPE, content_type("proto", text))
        .build()
        .expect("request")
}

fn roundtrip(text: bool) {
    let registry = CompressionRegistry::new();
    let (req_tx, req_rx) = Pipe::<Bytes>::with_depth(8);
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(8);

    let server = GrpcWebHandler
        .new_conn(
            &request(text),
            Spec {
                stream_type: StreamType::ServerStream,
                procedure: "svc.Svc/Echo".to_string(),
                is_client: false,
                idempotency: IdempotencyLevel::Unknown,
            },
            req_rx,
            resp_tx,
            &registry,
        )
        .expect("server exchange");
    // Head pipe for the mock client transport: this streaming test drives I/O
    // through the split conn and never reads the response head, so hand over an
    // open (empty) head receiver whose sender is parked for the test's lifetime.
    let (_client_head_tx, client_head_rx): (
        foundation_core::valtron::PipeSender<(
            foundation_netio::simple_http::shared::Status,
            foundation_netio::simple_http::shared::SimpleHeaders,
        )>,
        foundation_connectrpc::transport::HeadSource,
    ) = foundation_core::valtron::Pipe::with_depth(1);
    let client = GrpcWebClient { text }
        .new_conn(
            &Spec {
                stream_type: StreamType::ServerStream,
                procedure: "svc.Svc/Echo".to_string(),
                is_client: true,
                idempotency: IdempotencyLevel::Unknown,
            },
            SimpleHeaders::new(),
            TransportStream {
                send_body: req_tx,
                // F45 Part D: TransportStream now speaks FutureStream; bridge the
                // pipe-fed test harness through the pipe→FutureStream adapters.
                head: foundation_connectrpc::transport::head_stream_from_pipe(client_head_rx),
                recv_body: foundation_connectrpc::transport::body_stream_from_pipe(resp_rx),
                trailers: {
                    let (tx, rx) = Pipe::with_depth(1);
                    tx.close();
                    rx
                },
            },
            CancelSignal::new(),
        )
        .expect("client exchange");

    let (mut server_rx, server_tx) = server.conn.split();
    let (mut client_tx, mut client_rx) = client.conn.split();

    let received: Arc<Mutex<Vec<Bytes>>> = Arc::new(Mutex::new(Vec::new()));
    let terminal_err: Arc<Mutex<Option<Code>>> = Arc::new(Mutex::new(None));

    let cs: BoxFut = Box::pin(async move {
        client_tx.send(Bytes::from_static(b"hello")).await.unwrap();
        client_tx.close_send().await.unwrap();
    });

    let mut server_sender = server_tx;
    let srv: BoxFut = Box::pin(async move {
        while let Some(bytes) = server_rx.receive().await.unwrap() {
            server_sender.send(bytes).await.unwrap();
        }
        // Close with a terminal error to exercise the status trailer path.
        server_sender
            .close(
                Some(ConnectError::unavailable("later").into()),
                SimpleHeaders::new(),
            )
            .await
            .unwrap();
    });

    let recv = Arc::clone(&received);
    let terr = Arc::clone(&terminal_err);
    let cr: BoxFut = Box::pin(async move {
        loop {
            match client_rx.receive().await {
                Ok(Some(b)) => recv.lock().unwrap().push(b),
                Ok(None) => break,
                Err(trace) => {
                    *terr.lock().unwrap() = Some(trace.current_context().code());
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

    assert_eq!(&*received.lock().unwrap(), &[Bytes::from_static(b"hello")]);
    // The 0x80 trailer frame carried grpc-status=14 (Unavailable) → Err on client.
    assert_eq!(*terminal_err.lock().unwrap(), Some(Code::Unavailable));
}

#[test]
fn binary_streaming_roundtrip_with_status_trailer() {
    roundtrip(false);
}

#[test]
fn text_streaming_roundtrip_with_status_trailer() {
    roundtrip(true);
}
