//! The H3 proxy streams a large upstream body (spec 70 feature 70, E3).
//!
//! WHY: `h3_proxy::forward` collected the whole upstream response body into one
//! `Vec` before it answered the H3 client. A 16 MiB download then needed 16 MiB of
//! proxy memory, and the client saw no byte until the last one arrived (rule 4:
//! stream bodies, never buffer them).
//!
//! WHAT: a real QUIC client asks the proxy for a 16 MiB body from a plain
//! HTTP/1.1 backend. The proxy's `BufferedBytesGauge` counts the response bytes
//! it holds at once (read from the upstream and not yet taken by the H3 stream).
//! Its peak must stay under 1 MiB, and the SHA-256 of the received body must
//! match the one the backend sent.

#![cfg(feature = "quic")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::valtron::{initialize_pool, BlockingFutureExt, Stream};
use foundation_netio::http3::{H3Connection, H3Request};
use foundation_netio::quic::{client_config_trusting_pem, QuicDriver, QuinnBidiStream};
use foundation_proxy::config::{ServiceConfig, SslConfig};
use foundation_proxy::{ProxyConfig, ProxyServer};
use sha2::{Digest, Sha256};

const CERT: &[u8] = include_bytes!("fixtures/quic_cert.pem");
const KEY: &[u8] = include_bytes!("fixtures/quic_key.pem");
const CA: &[u8] = include_bytes!("fixtures/quic_ca.pem");

/// The size of the upstream body.
const BODY_LEN: usize = 16 * 1024 * 1024;
/// The most response bytes the proxy may hold at once.
const PEAK_LIMIT: usize = 1024 * 1024;
/// The hard time limit of the whole test.
const TIME_LIMIT: Duration = Duration::from_secs(120);

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// The upstream body: a fixed pseudo-random byte sequence (xorshift), so the test
/// computes its hash without keeping a copy.
fn body_chunk(state: &mut u64, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }
    out.truncate(len);
    out
}

fn expected_hash() -> [u8; 32] {
    let mut hasher = Sha256::new();
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut left = BODY_LEN;
    while left > 0 {
        let n = left.min(64 * 1024);
        hasher.update(body_chunk(&mut state, n));
        left -= n;
    }
    hasher.finalize().into()
}

/// A one-request HTTP/1.1 upstream that sends `BODY_LEN` bytes with a
/// `Content-Length`, in 64 KiB writes.
fn start_upstream() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind upstream");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        while let Ok((mut sock, _)) = listener.accept() {
            std::thread::spawn(move || {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    if sock.read(&mut byte).unwrap_or(0) == 0 {
                        return;
                    }
                    head.push(byte[0]);
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {BODY_LEN}\r\nConnection: close\r\n\r\n"
                );
                if sock.write_all(response.as_bytes()).is_err() {
                    return;
                }
                let mut state = 0x9E37_79B9_7F4A_7C15_u64;
                let mut left = BODY_LEN;
                while left > 0 {
                    let n = left.min(64 * 1024);
                    if sock.write_all(&body_chunk(&mut state, n)).is_err() {
                        return;
                    }
                    left -= n;
                }
            });
        }
    });
    port
}

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

fn send_frame(
    request: &mut H3Request<QuinnBidiStream>,
    mut pending: Bytes,
    headers: bool,
    deadline: Instant,
) {
    loop {
        let step = if headers {
            request.poll_send_headers(&mut pending)
        } else {
            request.poll_send_data(&mut pending)
        };
        if let Stream::Next(_) = step {
            return;
        }
        assert!(Instant::now() < deadline, "client send timed out");
        std::thread::sleep(ms(1));
    }
}

/// A fixed context over a real monotonic clock, for the client connection.
fn test_ctx() -> foundation_context::Ctx {
    foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    ))
}

#[test]
fn h3_forward_streams_large_body() {
    let _guard = initialize_pool(92, Some(8));
    let upstream_port = start_upstream();

    let dir = std::env::temp_dir().join(format!("ewe_h3_stream_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    std::fs::write(&cert_path, CERT).expect("cert");
    std::fs::write(&key_path, KEY).expect("key");

    let svc = ServiceConfig::new("big", "localhost")
        .backend(&format!("http://127.0.0.1:{upstream_port}"));
    let config = ProxyConfig::new("localhost", "127.0.0.1")
        .bind("127.0.0.1:0")
        .ssl(SslConfig::static_cert(
            &cert_path.display().to_string(),
            &key_path.display().to_string(),
        ))
        .h3_bind("127.0.0.1:0")
        .service(svc);
    let proxy = ProxyServer::start(config, foundation_context::Ctx::host())
        .block()
        .expect("start proxy");
    let h3_addr = proxy.h3_local_addr().expect("H3 front end bound");
    std::thread::sleep(ms(300));

    let started = Instant::now();
    let deadline = started + TIME_LIMIT;
    let client_cfg = client_config_trusting_pem(CA).expect("client config");
    let (driver, conn) =
        QuicDriver::connect(h3_addr, client_cfg, "localhost", &test_ctx()).expect("connect");
    foundation_core::valtron::send(driver).expect("spawn client driver");
    let mut h3 = H3Connection::new(conn);
    drive(|| h3.poll_setup(), deadline).expect("client h3 setup");
    let mut request = loop {
        match h3.poll_open_request() {
            Stream::Next(Ok(r)) => break r,
            Stream::Next(Err(e)) => panic!("open request: {e}"),
            _ => {
                assert!(Instant::now() < deadline, "never opened request stream");
                std::thread::sleep(ms(1));
            }
        }
    };
    let head = H3Request::<QuinnBidiStream>::encode_headers(&[
        (b":method".as_ref(), b"GET".as_ref()),
        (b":scheme".as_ref(), b"https".as_ref()),
        (b":authority".as_ref(), b"localhost".as_ref()),
        (b":path".as_ref(), b"/big".as_ref()),
    ]);
    send_frame(&mut request, head, true, deadline);
    loop {
        if let Stream::Next(_) = request.poll_finish() {
            break;
        }
        assert!(Instant::now() < deadline, "finish timed out");
        std::thread::sleep(ms(1));
    }

    let headers = drive(|| request.poll_headers(), deadline).expect("response headers");
    let status = headers
        .iter()
        .find(|(k, _)| k.as_ref() == b":status")
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .expect("status");
    assert_eq!(status, "200", "proxied H3 response status");

    let mut hasher = Sha256::new();
    let mut received = 0usize;
    loop {
        match request.poll_body() {
            Stream::Next(Ok(Some(chunk))) => {
                received += chunk.len();
                hasher.update(&chunk);
            }
            Stream::Next(Ok(None)) => break,
            Stream::Next(Err(e)) => panic!("body read failed after {received} bytes: {e}"),
            _ => {
                assert!(
                    Instant::now() < deadline,
                    "body read timed out after {received} bytes"
                );
                std::thread::sleep(ms(1));
            }
        }
    }

    let peak = proxy.state().h3_body_gauge().peak();
    let held_at_end = proxy.state().h3_body_gauge().current();
    eprintln!("peak body bytes held by the proxy: {peak}");
    proxy.shutdown();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(received, BODY_LEN, "the client got the whole body");
    let digest: [u8; 32] = hasher.finalize().into();
    assert_eq!(digest, expected_hash(), "SHA-256 of the streamed body");
    assert_eq!(held_at_end, 0, "the proxy let go of every byte it held");
    assert!(
        peak < PEAK_LIMIT,
        "the proxy held {peak} body bytes at once; the limit is {PEAK_LIMIT}"
    );
}
