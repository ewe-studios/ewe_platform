//! HTTP/3 request/response round-trip over real QUIC (F34 acceptance).
//!
//! WHY: F34's acceptance criterion is "HTTP/3 request/response round-trip …
//! produces/consumes the universal Simple types". Framing and QPACK unit tests do
//! not prove that: the control stream, the SETTINGS exchange, and the request
//! stream's HEADERS-then-DATA sequencing only interact over a live connection.
//!
//! WHAT: a client and a server, over loopback UDP, exchanging a real HTTP/3
//! request and response through [`H3Connection`] — with the request arriving as a
//! `SimpleIncomingRequestHeader` and the response leaving as a
//! `SimpleOutgoingResponse`.
//!
//! HOW: both `QuicDriver`s are `TaskIterator`s, so the test steps them by hand and
//! stays deterministic. `Stream::Pending` from any layer means "call me again",
//! which is exactly what `drive_until` does.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;

use foundation_core::valtron::{Stream, TaskIterator};
use foundation_netio::http3::connection::{error_code, H3Connection, H3Request};
use foundation_netio::http3::{request_from_fields, response_to_fields};
use foundation_netio::netcap::context::ConnectionContext;
use foundation_netio::quic::{
    client_config_trusting_pem, server_config_from_pem, QuicDriver, QuinnBidiStream,
    QuinnConnection,
};
use foundation_netio::simple_http::shared::{
    SimpleHeader, SimpleMethod, SimpleOutgoingResponse, Status,
};

const CERT_PEM: &[u8] = include_bytes!("../fixtures/quic_cert.pem");
const KEY_PEM: &[u8] = include_bytes!("../fixtures/quic_key.pem");
const CA_PEM: &[u8] = include_bytes!("../fixtures/quic_ca.pem");

const BUDGET: Duration = Duration::from_secs(5);

fn pump(server: &mut QuicDriver, client: &mut QuicDriver) {
    let _ = server.next_status();
    let _ = client.next_status();
}

/// Drive both QUIC drivers until `done` yields.
///
/// `done` takes the server driver, because the accept queue lives on it and a
/// closure capturing it would collide with the `&mut` the pump needs.
fn drive_until<T>(
    server: &mut QuicDriver,
    client: &mut QuicDriver,
    mut done: impl FnMut(&QuicDriver) -> Option<T>,
) -> Option<T> {
    let deadline = Instant::now() + BUDGET;
    while Instant::now() < deadline {
        if let Some(v) = done(server) {
            return Some(v);
        }
        pump(server, client);
    }
    None
}

/// Bring up a connected QUIC pair.
fn quic_pair() -> (QuicDriver, QuicDriver, QuinnConnection, QuinnConnection) {
    let server_cfg = server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config");
    let client_cfg = client_config_trusting_pem(CA_PEM).expect("client config");

    let mut server = QuicDriver::server("127.0.0.1:0".parse().unwrap(), server_cfg).expect("bind");
    let addr = server.local_addr().expect("addr");
    let (mut client, client_conn) =
        QuicDriver::connect(addr, client_cfg, "localhost").expect("connect");

    let server_conn = drive_until(&mut server, &mut client, QuicDriver::take_accepted)
        .expect("server accepted");

    (server, client, server_conn, client_conn)
}

/// Run `f` until it yields, driving both QUIC drivers between attempts.
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
            let _ = $server.next_status();
            let _ = $client.next_status();
        }
    }};
}

#[test]
fn http3_request_response_round_trip_over_the_simple_types() {
    let (mut sq, mut cq, server_conn, client_conn) = quic_pair();

    let mut server = H3Connection::new(server_conn);
    let mut client = H3Connection::new(client_conn);

    // Each side opens its control stream and sends SETTINGS first (RFC 9114 §6.2.1).
    until!(sq, cq, server.poll_setup());
    until!(sq, cq, client.poll_setup());

    // And each reads the other's SETTINGS.
    let ready = until!(sq, cq, client.poll_peer_settings());
    assert!(ready, "client must observe the server's SETTINGS");
    let ready = until!(sq, cq, server.poll_peer_settings());
    assert!(ready, "server must observe the client's SETTINGS");

    // The server told us its QPACK dynamic table is disabled — which is what lets
    // our QPACK omit the encoder/decoder streams entirely.
    let settings = client.peer_settings().expect("settings present");
    assert!(
        settings.iter().any(|(id, v)| *id == 0x01 && *v == 0),
        "QPACK_MAX_TABLE_CAPACITY must be 0: {settings:?}"
    );

    // ── Client sends a request ──
    let mut req: H3Request<QuinnBidiStream> = until!(sq, cq, client.poll_open_request());

    let request_fields: Vec<(&[u8], &[u8])> = vec![
        (b":method", b"POST"),
        (b":scheme", b"https"),
        (b":authority", b"example.com"),
        (b":path", b"/svc/Echo"),
        (b"content-type", b"application/grpc"),
    ];
    let mut headers = H3Request::<QuinnBidiStream>::encode_headers(&request_fields);
    until!(sq, cq, req.poll_send_headers(&mut headers));

    let mut data = H3Request::<QuinnBidiStream>::encode_data(Bytes::from_static(b"hello h3"));
    until!(sq, cq, req.poll_send_data(&mut data));
    until!(sq, cq, req.poll_finish());

    // ── Server accepts it ──
    let mut inbound = until!(sq, cq, server.poll_accept());
    let fields = until!(sq, cq, inbound.poll_headers());

    // The whole point: this is the same type HTTP/1.1 and HTTP/2 produce.
    let header = request_from_fields(&fields, Arc::new(ConnectionContext::default()))
        .expect("well-formed request");
    assert_eq!(header.method, SimpleMethod::POST);
    assert_eq!(header.path, "/svc/Echo");
    assert_eq!(header.authority, "example.com");
    assert_eq!(
        header
            .headers
            .get(&SimpleHeader::from("content-type".to_string()))
            .map(Vec::as_slice),
        Some(&["application/grpc".to_string()][..])
    );

    let body = until!(sq, cq, inbound.poll_body()).expect("a DATA frame");
    assert_eq!(&body[..], b"hello h3");

    let end = until!(sq, cq, inbound.poll_body());
    assert!(end.is_none(), "the client finished its request stream");

    // ── Server responds on the same stream ──
    let mut response = SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .build()
        .expect("response");
    response
        .headers
        .insert(SimpleHeader::from("content-type".to_string()), vec!["application/grpc".into()]);

    let response_fields = response_to_fields(&response);
    let pairs: Vec<(&[u8], &[u8])> =
        response_fields.iter().map(|(n, v)| (&n[..], &v[..])).collect();

    let mut resp_headers = H3Request::<QuinnBidiStream>::encode_headers(&pairs);
    until!(sq, cq, inbound.poll_send_headers(&mut resp_headers));

    let mut resp_data = H3Request::<QuinnBidiStream>::encode_data(Bytes::from_static(b"hello h3"));
    until!(sq, cq, inbound.poll_send_data(&mut resp_data));
    until!(sq, cq, inbound.poll_finish());

    // ── Client reads the response on the same stream ──
    let fields = until!(sq, cq, req.poll_headers());
    let status = fields
        .iter()
        .find(|(n, _)| &n[..] == b":status")
        .map(|(_, v)| v.clone())
        .expect(":status must be present and first");
    assert_eq!(&status[..], b"200", "the bare status code, not \"200 OK\"");

    let echoed = until!(sq, cq, req.poll_body()).expect("response DATA");
    assert_eq!(&echoed[..], b"hello h3", "the body must round-trip");

    let end = until!(sq, cq, req.poll_body());
    assert!(end.is_none(), "the server finished its response stream");
}

#[test]
fn a_request_stream_that_opens_with_data_is_a_frame_unexpected_error() {
    // RFC 9114 §4.1: a request stream opens with HEADERS. A DATA frame first means
    // the peer is not speaking HTTP/3, and answering it as if it were would let a
    // bodiless request through.
    let (mut sq, mut cq, server_conn, client_conn) = quic_pair();

    let mut server = H3Connection::new(server_conn);
    let mut client = H3Connection::new(client_conn);
    until!(sq, cq, server.poll_setup());
    until!(sq, cq, client.poll_setup());

    let mut req: H3Request<QuinnBidiStream> = until!(sq, cq, client.poll_open_request());
    let mut data = H3Request::<QuinnBidiStream>::encode_data(Bytes::from_static(b"body first"));
    until!(sq, cq, req.poll_send_data(&mut data));
    until!(sq, cq, req.poll_finish());

    let mut inbound = until!(sq, cq, server.poll_accept());

    let deadline = Instant::now() + BUDGET;
    let err = loop {
        match inbound.poll_headers() {
            Stream::Next(Err(e)) => break e,
            Stream::Next(Ok(_)) => panic!("DATA before HEADERS must not yield a request"),
            _ => {}
        }
        assert!(Instant::now() < deadline, "timed out");
        pump(&mut sq, &mut cq);
    };

    let text = err.to_string();
    assert!(
        text.contains(&format!("{:#06x}", error_code::H3_FRAME_UNEXPECTED)),
        "must be H3_FRAME_UNEXPECTED: {text}"
    );
}

#[test]
fn a_control_stream_whose_first_frame_is_not_settings_is_rejected() {
    // RFC 9114 §6.2.1: SETTINGS must be the first frame on the control stream.
    // Anything else is H3_MISSING_SETTINGS.
    use foundation_netio::http3::connection::stream_type;
    use foundation_netio::http3::frame::Frame;
    use foundation_netio::http3::VarInt;
    use foundation_netio::quic::{QuicConnection, QuicSendStream};

    let (mut sq, mut cq, server_conn, mut client_conn) = quic_pair();
    let mut server = H3Connection::new(server_conn);

    // Hand-roll a control stream that opens with GOAWAY instead of SETTINGS.
    let mut control = until!(sq, cq, client_conn.open_send());
    let mut preamble = Vec::new();
    VarInt::new(stream_type::CONTROL).unwrap().encode(&mut preamble);
    Frame::GoAway(VarInt::new(0).unwrap()).encode(&mut preamble);
    let mut bytes = Bytes::from(preamble);

    let deadline = Instant::now() + BUDGET;
    while bytes.len() > 0 {
        match control.send(&mut bytes) {
            Stream::Next(Ok(_)) => {}
            Stream::Next(Err(e)) => panic!("{e}"),
            _ => {}
        }
        assert!(Instant::now() < deadline, "timed out sending");
        pump(&mut sq, &mut cq);
    }

    let err = loop {
        match server.poll_peer_settings() {
            Stream::Next(Err(e)) => break e,
            Stream::Next(Ok(true)) => panic!("a GOAWAY-first control stream must be rejected"),
            _ => {}
        }
        assert!(Instant::now() < deadline, "timed out");
        pump(&mut sq, &mut cq);
    };

    let text = err.to_string();
    assert!(
        text.contains(&format!("{:#06x}", error_code::H3_MISSING_SETTINGS)),
        "must be H3_MISSING_SETTINGS: {text}"
    );
}

#[test]
fn a_trailing_headers_frame_is_surfaced_as_trailers() {
    // A second HEADERS frame after the body is the trailers section (RFC 9114
    // §4.1). gRPC puts its status there, so a transport that discarded it would
    // make every gRPC call over HTTP/3 fail to report its outcome.
    let (mut sq, mut cq, server_conn, client_conn) = quic_pair();

    let mut server = H3Connection::new(server_conn);
    let mut client = H3Connection::new(client_conn);
    until!(sq, cq, server.poll_setup());
    until!(sq, cq, client.poll_setup());

    let mut req: H3Request<QuinnBidiStream> = until!(sq, cq, client.poll_open_request());

    let head: Vec<(&[u8], &[u8])> = vec![
        (b":method", b"POST"),
        (b":scheme", b"https"),
        (b":path", b"/svc/Grpc"),
    ];
    let mut headers = H3Request::<QuinnBidiStream>::encode_headers(&head);
    until!(sq, cq, req.poll_send_headers(&mut headers));

    let mut data = H3Request::<QuinnBidiStream>::encode_data(Bytes::from_static(b"payload"));
    until!(sq, cq, req.poll_send_data(&mut data));

    // The trailers are just another HEADERS frame.
    let trailing: Vec<(&[u8], &[u8])> = vec![(b"grpc-status", b"0"), (b"grpc-message", b"ok")];
    let mut trailers = H3Request::<QuinnBidiStream>::encode_headers(&trailing);
    until!(sq, cq, req.poll_send_headers(&mut trailers));
    until!(sq, cq, req.poll_finish());

    let mut inbound = until!(sq, cq, server.poll_accept());
    let _ = until!(sq, cq, inbound.poll_headers());

    assert!(inbound.trailers().is_none(), "no trailers before the body is drained");

    let body = until!(sq, cq, inbound.poll_body()).expect("DATA");
    assert_eq!(&body[..], b"payload");

    let end = until!(sq, cq, inbound.poll_body());
    assert!(end.is_none(), "the trailers section ends the body");

    let trailers = inbound.trailers().expect("trailers must be surfaced, not discarded");
    assert!(
        trailers.iter().any(|(n, v)| &n[..] == b"grpc-status" && &v[..] == b"0"),
        "gRPC's status lives in the trailers: {trailers:?}"
    );
}
