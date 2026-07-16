//! End-to-end HTTP/3 server test (F19) — a real QUIC client talks to the
//! `foundation_http` H3 server over loopback UDP, and an echo `H3Serve` handler
//! reflects the request body. Proves the H3 serving layer works, not stubs.

#![cfg(feature = "quic")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::{initialize_pool, Stream};
use foundation_netio::http3::{H3Connection, H3Request};
use foundation_netio::quic::{client_config_trusting_pem, QuicDriver, QuinnBidiStream};

use foundation_http::native::serve::{BoxFuture, H3Serve};
use foundation_http::native::server::serve_h3;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_netio::netcap::ConnectionContext;

const CERT: &[u8] = include_bytes!("fixtures/quic_cert.pem");
const KEY: &[u8] = include_bytes!("fixtures/quic_key.pem");
const CA: &[u8] = include_bytes!("fixtures/quic_ca.pem");

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// Drive a poll-based H3 step to a terminal `Next(Ok/Err)` within `deadline`.
fn drive<T>(
    mut poll: impl FnMut() -> Stream<Result<T, foundation_netio::http3::H3Error>, ()>,
    deadline: Instant,
) -> Option<T> {
    loop {
        match poll() {
            Stream::Next(Ok(v)) => return Some(v),
            Stream::Next(Err(_)) => return None,
            _ => {
                if Instant::now() >= deadline {
                    return None;
                }
                std::thread::sleep(ms(1));
            }
        }
    }
}

/// Send a fully-encoded frame (`pending`) on a request's send half.
fn send_frame(request: &mut H3Request<QuinnBidiStream>, mut pending: Bytes, headers: bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let step = if headers {
            request.poll_send_headers(&mut pending)
        } else {
            request.poll_send_data(&mut pending)
        };
        match step {
            Stream::Next(Ok(())) => return,
            Stream::Next(Err(_)) => return,
            _ => {
                if Instant::now() >= deadline {
                    return;
                }
                std::thread::sleep(ms(1));
            }
        }
    }
}

/// Read the whole response/request body until end-of-stream.
fn read_body(request: &mut H3Request<QuinnBidiStream>, deadline: Instant) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        match request.poll_body() {
            Stream::Next(Ok(Some(chunk))) => body.extend_from_slice(&chunk),
            Stream::Next(Ok(None)) => return body,
            Stream::Next(Err(_)) => return body,
            _ => {
                if Instant::now() >= deadline {
                    return body;
                }
                std::thread::sleep(ms(1));
            }
        }
    }
}

// ── Echo H3 handler ──────────────────────────────────────────────────────

struct EchoH3;

impl H3Serve for EchoH3 {
    fn serve_h3(
        &self,
        _bag: Arc<ContextBag>,
        _connection: Arc<ConnectionContext>,
        mut request: H3Request<QuinnBidiStream>,
    ) -> BoxFuture<'static, std::io::Result<()>> {
        Box::pin(async move {
            let deadline = Instant::now() + Duration::from_secs(10);
            // Read request headers + body.
            let _headers = drive(|| request.poll_headers(), deadline);
            let body = read_body(&mut request, deadline);

            // Respond 200 echoing the body.
            let resp_headers =
                H3Request::<QuinnBidiStream>::encode_headers(&[(b":status".as_ref(), b"200".as_ref())]);
            send_frame(&mut request, resp_headers, true);
            let data = H3Request::<QuinnBidiStream>::encode_data(Bytes::from(body));
            send_frame(&mut request, data, false);

            // Finish the send half.
            let fin_deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match request.poll_finish() {
                    Stream::Next(Ok(())) | Stream::Next(Err(_)) => break,
                    _ => {
                        if Instant::now() >= fin_deadline {
                            break;
                        }
                        std::thread::sleep(ms(1));
                    }
                }
            }
            Ok(())
        })
    }
}

// ── Test ─────────────────────────────────────────────────────────────────

#[test]
fn http3_server_echoes_a_request_over_quic() {
    let _guard = initialize_pool(77, Some(8));

    // Start the H3 server with the echo handler.
    let mut app = HttpApp::new_h3_serve();
    app.route_any_h3("/*", Arc::new(EchoH3));
    let shutdown = Arc::new(OnSignal::new());
    let server_addr = serve_h3(
        "127.0.0.1:0".parse().unwrap(),
        CERT,
        KEY,
        Arc::new(app),
        Arc::clone(&shutdown),
    )
    .expect("start h3 server");

    // Give the driver/accept tasks a moment to be scheduled.
    std::thread::sleep(ms(200));

    // ── Client: connect over QUIC and drive one H3 request. ──
    let client_cfg = client_config_trusting_pem(CA).expect("client config");
    let (driver, conn) = QuicDriver::connect(server_addr, client_cfg, "localhost").expect("connect");
    foundation_core::valtron::send(driver).expect("spawn client driver");

    let mut h3 = H3Connection::new(conn);
    let deadline = Instant::now() + Duration::from_secs(15);
    drive(|| h3.poll_setup(), deadline).expect("client h3 setup");

    let mut request = {
        let mut req = None;
        while Instant::now() < deadline {
            match h3.poll_open_request() {
                Stream::Next(Ok(r)) => {
                    req = Some(r);
                    break;
                }
                Stream::Next(Err(e)) => panic!("open request: {e}"),
                _ => std::thread::sleep(ms(1)),
            }
        }
        req.expect("client opened a request stream")
    };

    let req_headers = H3Request::<QuinnBidiStream>::encode_headers(&[
        (b":method".as_ref(), b"POST".as_ref()),
        (b":scheme".as_ref(), b"https".as_ref()),
        (b":authority".as_ref(), b"localhost".as_ref()),
        (b":path".as_ref(), b"/echo".as_ref()),
    ]);
    send_frame(&mut request, req_headers, true);
    send_frame(&mut request, H3Request::<QuinnBidiStream>::encode_data(Bytes::from_static(b"hello-http3")), false);
    // Finish the request send half so the server sees end-of-body.
    let fin_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match request.poll_finish() {
            Stream::Next(Ok(())) | Stream::Next(Err(_)) => break,
            _ => {
                if Instant::now() >= fin_deadline {
                    break;
                }
                std::thread::sleep(ms(1));
            }
        }
    }

    // Read the response.
    let headers = drive(|| request.poll_headers(), deadline).expect("response headers");
    let status = headers
        .iter()
        .find(|(k, _)| k.as_ref() == b":status")
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .expect("status header");
    assert_eq!(status, "200", "expected 200 from echo handler");

    let body = read_body(&mut request, deadline);
    assert_eq!(&body, b"hello-http3", "echoed body: {:?}", String::from_utf8_lossy(&body));

    shutdown.turn_on();
}
