//! HTTP/3 ConnectRPC conformance — unary over live QUIC (F35).
//!
//! WHY: F35 requires Connect+gRPC to pass over HTTP/3. This exercises the
//! server half end-to-end: a Connect unary request on a live QUIC pair,
//! dispatched through `dispatch_h3`, verified client-side.
//!
//! HOW: quic_pair() + H3Connection. dispatch_h3 is polled manually alongside
//! QUIC driver pumps — no executor, no tokio, no thread spawning needed.

#[cfg(feature = "h3")]
mod h3_conformance {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    use std::time::{Duration, Instant};

    use bytes::Bytes;
    use foundation_core::valtron::{Stream as VStream, TaskIterator};
    use foundation_http::shared::context::ContextBag;
    use foundation_netio::http3::connection::{H3Connection, H3Request};
    use foundation_netio::quic::{
        client_config_trusting_pem, server_config_from_pem, QuicDriver, QuinnBidiStream,
        QuinnConnection,
    };
    use foundation_connectrpc::{
        Ctx, HandlerOptions, JsonCodec, ProcedureCodecs, Request, Response, Router,
    };
    use foundation_connectrpc::native::h3_serve::dispatch_h3;

    const CERT_PEM: &[u8] = include_bytes!("../../foundation_netio/tests/fixtures/quic_cert.pem");
    const KEY_PEM: &[u8] = include_bytes!("../../foundation_netio/tests/fixtures/quic_key.pem");
    const CA_PEM: &[u8] = include_bytes!("../../foundation_netio/tests/fixtures/quic_ca.pem");
    const BUDGET: Duration = Duration::from_secs(10);

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct EchoMsg { msg: String }

    fn codecs() -> ProcedureCodecs<EchoMsg, EchoMsg> {
        ProcedureCodecs::<EchoMsg, EchoMsg>::of((JsonCodec,))
    }

    fn pump(sq: &mut QuicDriver, cq: &mut QuicDriver) {
        let _ = sq.next_status();
        let _ = cq.next_status();
    }

    fn quic_pair() -> (QuicDriver, QuicDriver, QuinnConnection, QuinnConnection) {
        let server_cfg = server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config");
        let client_cfg = client_config_trusting_pem(CA_PEM).expect("client config");
        let mut sq = QuicDriver::server("127.0.0.1:0".parse().unwrap(), server_cfg).expect("bind");
        let addr = sq.local_addr().expect("addr");
        let (mut cq, cc) = QuicDriver::connect(addr, client_cfg, "localhost").expect("connect");
        let dl = Instant::now() + BUDGET;
        let sc = loop {
            if let Some(c) = QuicDriver::take_accepted(&sq) { break c; }
            pump(&mut sq, &mut cq);
            assert!(Instant::now() < dl, "timed out waiting for accept");
        };
        (sq, cq, sc, cc)
    }

    macro_rules! until {
        ($sq:expr, $cq:expr, $e:expr) => {{
            let dl = Instant::now() + BUDGET;
            loop {
                match $e {
                    VStream::Next(Ok(v)) => break v,
                    VStream::Next(Err(e)) => panic!("{e}"),
                    _ => {}
                }
                assert!(Instant::now() < dl, "timed out driving HTTP/3");
                pump($sq, $cq);
            }
        }};
    }

    fn noop_waker() -> Waker {
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        fn wake(_: *const ()) {}
        fn wake_by_ref(_: *const ()) {}
        fn drop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
    }

    /// Poll `fut` alongside QUIC pumps. Returns `(output, sq, cq)` so the
    /// drivers remain available for the client to read the response.
    fn drive_dispatch<F: Future>(
        mut sq: QuicDriver,
        mut cq: QuicDriver,
        fut: F,
    ) -> (F::Output, QuicDriver, QuicDriver) {
        futures::pin_mut!(fut);
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);
        let dl = Instant::now() + BUDGET;
        let result = loop {
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(v) => break v,
                Poll::Pending => {
                    pump(&mut sq, &mut cq);
                    assert!(Instant::now() < dl, "timed out polling dispatch_h3");
                }
            }
        };
        (result, sq, cq)
    }

    #[test]
    fn connect_unary_echo_over_h3() {
        let mut router = Router::new();
        router.unary(
            "/echo.EchoService/Echo",
            codecs(),
            |_ctx: Ctx, req: Request<EchoMsg>| async move {
                Ok(Response::new(req.into_message()))
            },
            HandlerOptions::new(),
        );
        let handler = Arc::new(router.into_handler());

        let (mut sq, mut cq, server_conn, client_conn) = quic_pair();
        let mut server = H3Connection::new(server_conn);
        let mut client = H3Connection::new(client_conn);
        until!(&mut sq, &mut cq, server.poll_setup());
        until!(&mut sq, &mut cq, client.poll_setup());
        until!(&mut sq, &mut cq, client.poll_peer_settings());
        until!(&mut sq, &mut cq, server.poll_peer_settings());

        // Client → request.
        let mut req: H3Request<QuinnBidiStream> = until!(&mut sq, &mut cq, client.poll_open_request());
        let fields: Vec<(&[u8], &[u8])> = vec![
            (b":method", b"POST"), (b":scheme", b"https"),
            (b":authority", b"localhost"), (b":path", b"/echo.EchoService/Echo"),
            (b"content-type", b"application/connect+json"),
        ];
        let mut hdrs = H3Request::<QuinnBidiStream>::encode_headers(&fields);
        until!(&mut sq, &mut cq, req.poll_send_headers(&mut hdrs));

        // Connect unary POST: bare JSON body (no envelope framing — that's gRPC).
        let msg = serde_json::to_vec(&EchoMsg { msg: "hello-h3".into() }).unwrap();
        let mut data = H3Request::<QuinnBidiStream>::encode_data(Bytes::from(msg));
        until!(&mut sq, &mut cq, req.poll_send_data(&mut data));
        until!(&mut sq, &mut cq, req.poll_finish());

        // Server accepts and dispatches.
        let inbound = until!(&mut sq, &mut cq, server.poll_accept());
        let bag = Arc::new(ContextBag::default());

        let (result, mut sq, mut cq) = drive_dispatch(sq, cq, dispatch_h3(handler, bag, inbound));
        result.expect("dispatch_h3 failed");

        // Client reads response.
        let fields = until!(&mut sq, &mut cq, req.poll_headers());
        let st = fields.iter().find(|(n, _)| &n[..] == b":status").map(|(_, v)| v.clone())
            .expect(":status must be present");
        assert_eq!(&st[..], b"200");

        let mut resp_body = Vec::new();
        loop {
            match until!(&mut sq, &mut cq, req.poll_body()) {
                Some(c) => resp_body.extend_from_slice(&c),
                None => break,
            }
        }
        // Connect unary response: raw JSON in the body (no envelope prefix).
        let echoed: EchoMsg = serde_json::from_slice(&resp_body).expect("valid JSON");
        assert_eq!(echoed.msg, "hello-h3");
    }
}
