//! `H3Connection::poll_accept_webtransport` must never lose an accepted stream.
//!
//! It used to drop the stream it had accepted whenever that stream's HEADERS
//! were not readable yet (or the `200` could not be queued at once), and it
//! answered an ordinary request with `Ok(false)` after having read — and thrown
//! away — its HEADERS.

use std::time::{Duration, Instant};

use bytes::Bytes;

use foundation_core::valtron::{Ctx, Stream, TaskIterator};
use foundation_netio::http3::connection::{H3Connection, H3Request};
use foundation_netio::http3::WtAccept;
use foundation_netio::quic::{
    client_config_trusting_pem, server_config_from_pem, QuicDriver, QuinnBidiStream,
    QuinnConnection,
};

const CERT_PEM: &[u8] = include_bytes!("../fixtures/quic_cert.pem");
const KEY_PEM: &[u8] = include_bytes!("../fixtures/quic_key.pem");
const CA_PEM: &[u8] = include_bytes!("../fixtures/quic_ca.pem");

const BUDGET: Duration = Duration::from_secs(5);

macro_rules! until {
    ($server:expr, $client:expr, $expr:expr) => {{
        let deadline = Instant::now() + BUDGET;
        loop {
            match $expr {
                Stream::Next(Ok(v)) => break v,
                Stream::Next(Err(e)) => panic!("{e}"),
                _ => {}
            }
            assert!(Instant::now() < deadline, "timed out driving HTTP/3");
            let _ = $server.next_status(Ctx::new());
            let _ = $client.next_status(Ctx::new());
        }
    }};
}

/// A connected, set-up HTTP/3 pair.
fn h3_pair() -> (
    QuicDriver,
    QuicDriver,
    H3Connection<QuinnConnection>,
    H3Connection<QuinnConnection>,
) {
    let server_cfg = server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config");
    let client_cfg = client_config_trusting_pem(CA_PEM).expect("client config");
    let mut sq = QuicDriver::server("127.0.0.1:0".parse().unwrap(), server_cfg).expect("bind");
    let addr = sq.local_addr().expect("addr");
    let (mut cq, client_conn) =
        QuicDriver::connect(addr, client_cfg, "localhost").expect("connect");

    let deadline = Instant::now() + BUDGET;
    let server_conn = loop {
        if let Some(conn) = sq.take_accepted() {
            break conn;
        }
        assert!(Instant::now() < deadline, "server never accepted");
        let _ = sq.next_status(Ctx::new());
        let _ = cq.next_status(Ctx::new());
    };

    let mut server = H3Connection::new(server_conn);
    let mut client = H3Connection::new(client_conn);
    until!(sq, cq, server.poll_setup());
    until!(sq, cq, client.poll_setup());
    until!(sq, cq, client.poll_peer_settings());
    until!(sq, cq, server.poll_peer_settings());
    (sq, cq, server, client)
}

fn send(
    sq: &mut QuicDriver,
    cq: &mut QuicDriver,
    req: &mut H3Request<QuinnBidiStream>,
    mut bytes: Bytes,
) {
    until!(sq, cq, req.poll_send_headers(&mut bytes));
}

fn header<'a>(headers: &'a [(Bytes, Bytes)], name: &[u8]) -> Option<&'a [u8]> {
    headers
        .iter()
        .find(|(k, _)| k.as_ref() == name)
        .map(|(_, v)| v.as_ref())
}

/// The stream arrives before its HEADERS (a reserved, unknown frame goes
/// first, RFC 9114 §7.2.8): the first polls see the stream but no HEADERS yet.
/// The stream must still be there when they arrive.
#[test]
fn a_stream_accepted_before_its_headers_is_not_lost() {
    let (mut sq, mut cq, mut server, mut client) = h3_pair();
    let mut req = until!(sq, cq, client.poll_open_request());

    // A reserved frame type (0x21 + 0x1f * N), empty payload.
    send(
        &mut sq,
        &mut cq,
        &mut req,
        Bytes::from_static(&[0x21, 0x00]),
    );
    // Let the server accept the stream and find no HEADERS yet.
    let deadline = Instant::now() + Duration::from_millis(300);
    while Instant::now() < deadline {
        assert!(
            !matches!(server.poll_accept_webtransport(), Stream::Next(_)),
            "nothing is complete yet"
        );
        let _ = sq.next_status(Ctx::new());
        let _ = cq.next_status(Ctx::new());
    }

    let fields: Vec<(&[u8], &[u8])> = vec![
        (b":method", b"GET"),
        (b":scheme", b"https"),
        (b":authority", b"example.com"),
        (b":path", b"/late"),
    ];
    send(
        &mut sq,
        &mut cq,
        &mut req,
        H3Request::<QuinnBidiStream>::encode_headers(&fields),
    );
    until!(sq, cq, req.poll_finish());

    match until!(sq, cq, server.poll_accept_webtransport()) {
        WtAccept::Request { headers, .. } => {
            assert_eq!(header(&headers, b":path"), Some(&b"/late"[..]));
        }
        WtAccept::Session => panic!("a GET is not a WebTransport CONNECT"),
    }
}

/// An ordinary request comes back with its HEADERS and its body still
/// readable; it used to be answered `Ok(false)` and dropped.
#[test]
fn an_ordinary_request_is_handed_back_with_its_headers() {
    let (mut sq, mut cq, mut server, mut client) = h3_pair();
    let mut req = until!(sq, cq, client.poll_open_request());
    let fields: Vec<(&[u8], &[u8])> = vec![
        (b":method", b"POST"),
        (b":scheme", b"https"),
        (b":authority", b"example.com"),
        (b":path", b"/echo"),
    ];
    send(
        &mut sq,
        &mut cq,
        &mut req,
        H3Request::<QuinnBidiStream>::encode_headers(&fields),
    );
    let mut data = H3Request::<QuinnBidiStream>::encode_data(Bytes::from_static(b"body"));
    until!(sq, cq, req.poll_send_data(&mut data));
    until!(sq, cq, req.poll_finish());

    let WtAccept::Request {
        mut request,
        headers,
    } = until!(sq, cq, server.poll_accept_webtransport())
    else {
        panic!("a POST is not a WebTransport CONNECT");
    };
    assert_eq!(header(&headers, b":method"), Some(&b"POST"[..]));
    let chunk = until!(sq, cq, request.poll_body());
    assert_eq!(chunk.as_deref(), Some(&b"body"[..]));
}

/// A WebTransport Extended CONNECT is answered `200` and reported as a session.
#[test]
fn a_webtransport_connect_is_answered_and_reported() {
    let (mut sq, mut cq, mut server, mut client) = h3_pair();
    let mut req = until!(sq, cq, client.poll_open_request());
    let fields: Vec<(&[u8], &[u8])> = vec![
        (b":method", b"CONNECT"),
        (b":protocol", b"webtransport"),
        (b":scheme", b"https"),
        (b":authority", b"example.com"),
        (b":path", b"/wt"),
    ];
    send(
        &mut sq,
        &mut cq,
        &mut req,
        H3Request::<QuinnBidiStream>::encode_headers(&fields),
    );

    assert!(matches!(
        until!(sq, cq, server.poll_accept_webtransport()),
        WtAccept::Session
    ));
    let response = until!(sq, cq, req.poll_headers());
    assert_eq!(header(&response, b":status"), Some(&b"200"[..]));
}
