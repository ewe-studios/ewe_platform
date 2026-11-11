//! Transport-seam tests (spec-41 F17 / Decision 11): capability matching, client
//! end-of-stream semantics, cancel-woken parked receive, and a bidi deadlock
//! stress test over split conn halves at depth 4.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::valtron::valtron_test;
use foundation_netio::shared::http::{Proto, SimpleHeader, SimpleHeaders};

use foundation_connectrpc::shared::context::{CancelSignal, Peer, Spec, StreamType};
use foundation_connectrpc::shared::transport::wasm::WASM_FETCH_CAPABILITIES;
use foundation_connectrpc::shared::transport::{
    check_compatible, requirements, ClientConn, Frame, HandlerConn, PipeClientConn,
    PipeHandlerConn, ProtocolKind, TransportCapabilities,
};

// ── tiny async test harness ───────────────────────────────────────────────────

struct ThreadWaker(std::thread::Thread);
impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(fut: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

type BoxFut = Pin<Box<dyn Future<Output = ()>>>;

/// Drive many futures to completion concurrently (round-robin), parking between
/// cycles and re-polling on a short timeout so no wakeup is ever lost. Panics if
/// the whole set fails to complete within the deadline — that is a deadlock.
fn run_all(mut futs: Vec<BoxFut>) {
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !futs.is_empty() {
        let mut i = 0;
        while i < futs.len() {
            match futs[i].as_mut().poll(&mut cx) {
                Poll::Ready(()) => {
                    drop(futs.swap_remove(i));
                }
                Poll::Pending => i += 1,
            }
        }
        if futs.is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "run_all timed out with {} futures outstanding — likely a deadlock",
            futs.len()
        );
        std::thread::park_timeout(Duration::from_millis(5));
    }
}

fn spec(stream: StreamType) -> Spec {
    Spec {
        stream_type: stream,
        procedure: "test.Svc/M".to_string(),
        is_client: false,
        idempotency: foundation_connectrpc::IdempotencyLevel::Unknown,
    }
}

// ── capability matching ───────────────────────────────────────────────────────

const H1: TransportCapabilities = TransportCapabilities {
    request_streaming: true,
    full_duplex: false,
    h2_trailers: false,
    http_versions: &[Proto::HTTP11],
    multiplexed: false,
};
const H2: TransportCapabilities = TransportCapabilities {
    request_streaming: true,
    full_duplex: true,
    h2_trailers: true,
    http_versions: &[Proto::HTTP20, Proto::HTTP30],
    multiplexed: true,
};
const FETCH: TransportCapabilities = TransportCapabilities {
    request_streaming: false,
    full_duplex: false,
    h2_trailers: false,
    http_versions: &[Proto::HTTP11, Proto::HTTP20],
    multiplexed: false,
};

fn ok(p: ProtocolKind, s: StreamType, cap: &TransportCapabilities) -> bool {
    check_compatible(&requirements(p, s), cap).is_ok()
}

#[test]
fn capability_matrix_http1() {
    // Connect/gRPC-Web unary + half-duplex streaming OK on HTTP/1.1; bidi rejected (505).
    assert!(ok(ProtocolKind::Connect, StreamType::Unary, &H1));
    assert!(ok(ProtocolKind::Connect, StreamType::ClientStream, &H1));
    assert!(ok(ProtocolKind::Connect, StreamType::ServerStream, &H1));
    assert!(!ok(ProtocolKind::Connect, StreamType::BidiStream, &H1));
    // gRPC needs HTTP/2 trailers + version floor → rejected on HTTP/1.1.
    assert!(!ok(ProtocolKind::Grpc, StreamType::Unary, &H1));
}

#[test]
fn capability_matrix_http2() {
    // Everything (incl. bidi + gRPC) allowed on HTTP/2.
    for s in [
        StreamType::Unary,
        StreamType::ClientStream,
        StreamType::ServerStream,
        StreamType::BidiStream,
    ] {
        assert!(ok(ProtocolKind::Connect, s, &H2), "connect {s:?}");
        assert!(ok(ProtocolKind::Grpc, s, &H2), "grpc {s:?}");
    }
}

#[test]
fn capability_matrix_wasm_fetch() {
    // Unary + server-streaming allowed; client-stream / bidi rejected (no request streaming).
    assert!(ok(ProtocolKind::Connect, StreamType::Unary, &FETCH));
    assert!(ok(ProtocolKind::GrpcWeb, StreamType::ServerStream, &FETCH));
    assert!(!ok(ProtocolKind::Connect, StreamType::ClientStream, &FETCH));
    assert!(!ok(ProtocolKind::Connect, StreamType::BidiStream, &FETCH));
}

#[test]
fn wasm_fetch_capabilities_are_correct() {
    let caps = WASM_FETCH_CAPABILITIES;
    assert!(!caps.request_streaming);
    assert!(!caps.full_duplex);
    assert!(!caps.h2_trailers);
    assert!(!caps.multiplexed);
}

// ── client end-of-stream semantics ────────────────────────────────────────────

#[test]
fn client_receive_errors_on_terminal_endstream() {
    let (conn, ends) = PipeClientConn::with_defaults(
        spec(StreamType::ServerStream),
        SimpleHeaders::new(),
        CancelSignal::new(),
    );
    let (_tx, mut rx) = Box::new(conn).split();

    // Reader pushes a terminal error end-stream.
    let err = foundation_connectrpc::ConnectError::not_found("gone").into();
    block_on(async {
        ends.response_tx
            .send(Frame::EndStream {
                error: Some(err),
                trailers: SimpleHeaders::new(),
            })
            .await
            .unwrap();
    });

    let outcome = block_on(rx.receive());
    let trace = outcome.expect_err("terminal error must surface as Err");
    assert_eq!(
        trace.current_context().code(),
        foundation_connectrpc::Code::NotFound
    );
}

#[test]
fn client_receive_clean_eos_is_ok_none() {
    let (conn, ends) = PipeClientConn::with_defaults(
        spec(StreamType::ServerStream),
        SimpleHeaders::new(),
        CancelSignal::new(),
    );
    let (_tx, mut rx) = Box::new(conn).split();

    block_on(async {
        ends.response_tx
            .send(Frame::Message(Bytes::from_static(b"one")))
            .await
            .unwrap();
        ends.response_tx
            .send(Frame::EndStream {
                error: None,
                trailers: SimpleHeaders::new(),
            })
            .await
            .unwrap();
    });

    assert_eq!(
        block_on(rx.receive()).unwrap(),
        Some(Bytes::from_static(b"one"))
    );
    assert_eq!(block_on(rx.receive()).unwrap(), None, "clean EOS");
}

// ── cancel wakes a parked receive ─────────────────────────────────────────────

#[test]
fn parked_receive_wakes_on_cancel() {
    let cancel = CancelSignal::new();
    let (conn, _ends) = PipeHandlerConn::with_defaults(
        spec(StreamType::ClientStream),
        Peer::empty(),
        SimpleHeaders::new(),
        cancel.clone(),
    );
    let (mut rx, _tx) = Box::new(conn).split();

    // No frames will ever arrive; a background thread fires cancel.
    let firer = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        firer.cancel();
    });

    let outcome = block_on(rx.receive());
    let trace = outcome.expect_err("cancel must interrupt the parked receive");
    assert_eq!(
        trace.current_context().code(),
        foundation_connectrpc::Code::Canceled
    );
}

// ── response head publication ─────────────────────────────────────────────────

/// A published head resolves the wait with no response frame.
///
/// The reader publishes the head as soon as it arrives. A peer that sends no
/// body frame must not keep the call setup open. Before feature 135 the wait
/// took the next response frame instead, so this test timed out.
#[valtron_test(timeout = 2_000)]
async fn response_headers_resolves_when_the_head_is_published_without_a_frame() {
    let (conn, ends) = PipeClientConn::with_defaults(
        spec(StreamType::ServerStream),
        SimpleHeaders::new(),
        CancelSignal::new(),
    );
    let (_tx, mut rx) = Box::new(conn).split();

    let mut head = SimpleHeaders::new();
    head.insert(
        SimpleHeader::from("x-head".to_string()),
        vec!["published".to_string()],
    );
    let expected = head.clone();

    // Publish after a short delay. No response frame ever arrives.
    let publish = async move {
        foundation_core::valtron::sleep_async(
            Duration::from_millis(50),
            &foundation_core::valtron::Ctx::host(),
        )
        .await;
        ends.publish_response_headers(head);
    };

    let (got, ()) = futures::join!(rx.response_headers(), publish);
    let got = got.expect("a published head must resolve the wait").clone();
    assert_eq!(got, expected);
}

/// A transport that ends without a head yields empty headers.
///
/// Dropping the ends closes the wake pipe, which is the signal that no head
/// will ever arrive. The wait must end rather than park for a frame.
#[valtron_test(timeout = 2_000)]
async fn response_headers_resolves_to_empty_when_the_transport_drops_without_a_head() {
    let (conn, ends) = PipeClientConn::with_defaults(
        spec(StreamType::ServerStream),
        SimpleHeaders::new(),
        CancelSignal::new(),
    );
    let (_tx, mut rx) = Box::new(conn).split();

    // The transport ends with no head and no frame.
    drop(ends);

    let headers = rx
        .response_headers()
        .await
        .expect("a failed transport must still yield a head")
        .clone();
    assert!(headers.is_empty(), "a failed request has no head");
}

/// Cancel ends the head wait with the call's usual error.
#[valtron_test(timeout = 2_000)]
async fn response_headers_stops_on_cancel() {
    let cancel = CancelSignal::new();
    let (conn, ends) = PipeClientConn::with_defaults(
        spec(StreamType::ServerStream),
        SimpleHeaders::new(),
        cancel.clone(),
    );
    let (_tx, mut rx) = Box::new(conn).split();
    // Keep the wake pipe open: it must not close as if the transport ended.
    let _ends = ends;

    let fire = async move {
        foundation_core::valtron::sleep_async(
            Duration::from_millis(50),
            &foundation_core::valtron::Ctx::host(),
        )
        .await;
        cancel.cancel();
    };

    let (result, ()) = futures::join!(rx.response_headers(), fire);
    match result {
        Ok(_) => panic!("cancel must interrupt the head wait"),
        Err(trace) => assert_eq!(
            trace.current_context().code(),
            foundation_connectrpc::Code::Canceled
        ),
    }
}

// ── bidi deadlock stress (split halves, depth 4) ──────────────────────────────

#[test]
fn bidi_concurrent_send_receive_no_deadlock_depth_4() {
    const DEPTH: usize = 4;
    const N: usize = 20; // well over the pipe depth → forces parking/backpressure

    let cancel = CancelSignal::new();
    let (server_conn, server_ends) = PipeHandlerConn::new(
        spec(StreamType::BidiStream),
        Peer::empty(),
        SimpleHeaders::new(),
        cancel.clone(),
        DEPTH,
    );
    let (client_conn, client_ends) = PipeClientConn::new(
        spec(StreamType::BidiStream),
        SimpleHeaders::new(),
        cancel,
        DEPTH,
    );

    let (mut server_rx, server_tx) = Box::new(server_conn).split();
    let (mut client_tx, mut client_rx) = Box::new(client_conn).split();

    let received = Arc::new(AtomicUsize::new(0));

    // Client send half: fire N requests, then half-close. Parks when the request
    // pipe fills (server not yet drained) — must not deadlock the receive half.
    let client_send: BoxFut = Box::pin(async move {
        for i in 0..N {
            client_tx
                .send(Bytes::from(format!("req-{i}")))
                .await
                .unwrap();
        }
        client_tx.close_send().await.unwrap();
    });

    // Client receive half: drain all echoes concurrently with sending.
    let recv_counter = Arc::clone(&received);
    let client_recv: BoxFut = Box::pin(async move {
        while client_rx.receive().await.unwrap().is_some() {
            recv_counter.fetch_add(1, Ordering::SeqCst);
        }
    });

    // Server: echo each request back, then close. Uses split halves too.
    let mut server_sender = server_tx;
    let server: BoxFut = Box::pin(async move {
        while let Some(bytes) = server_rx.receive().await.unwrap() {
            server_sender.send(bytes).await.unwrap();
        }
        server_sender
            .close(None, SimpleHeaders::new())
            .await
            .unwrap();
    });

    // Pumps wiring client ⇄ server byte-for-byte (stand-in for the wire).
    let req_pump_src = client_ends.request_rx;
    let req_pump_dst = server_ends.request_tx;
    let request_pump: BoxFut = Box::pin(async move {
        while let Some(frame) = req_pump_src.receive().await {
            if req_pump_dst.send(frame).await.is_err() {
                break;
            }
        }
        let _ = req_pump_dst.close();
    });

    let resp_pump_src = server_ends.response_rx;
    let resp_pump_dst = client_ends.response_tx;
    let response_pump: BoxFut = Box::pin(async move {
        while let Some(frame) = resp_pump_src.receive().await {
            if resp_pump_dst.send(frame).await.is_err() {
                break;
            }
        }
        let _ = resp_pump_dst.close();
    });

    run_all(vec![
        client_send,
        client_recv,
        server,
        request_pump,
        response_pump,
    ]);

    assert_eq!(
        received.load(Ordering::SeqCst),
        N,
        "all echoes received without deadlock"
    );
}
