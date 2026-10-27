//! HTTP/3 futures park on the QUIC connection instead of busy-polling.
//!
//! QUIC streams used to register no waker: every not-ready H3 poll had to wake
//! its own task, so an idle request kept a worker spinning. Now a caller
//! registers its waker on the connection and the endpoint driver wakes it when
//! the connection makes progress; the driver also wakes accept loops when a
//! connection is handshaked.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Wake, Waker};
use std::time::{Duration, Instant};

use bytes::Bytes;

use foundation_core::valtron::{Ctx, Stream, TaskIterator};
use foundation_netio::http3::connection::{H3Connection, H3Request};
use foundation_netio::quic::{
    client_config_trusting_pem, server_config_from_pem, QuicDriver, QuinnBidiStream,
};

const CERT_PEM: &[u8] = include_bytes!("../fixtures/quic_cert.pem");
const KEY_PEM: &[u8] = include_bytes!("../fixtures/quic_key.pem");
const CA_PEM: &[u8] = include_bytes!("../fixtures/quic_ca.pem");
const BUDGET: Duration = Duration::from_secs(5);

/// Counts its wakes.
#[derive(Default)]
struct CountingWaker(AtomicUsize);

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn pump(sq: &mut QuicDriver, cq: &mut QuicDriver) {
    let _ = sq.next_status(Ctx::new());
    let _ = cq.next_status(Ctx::new());
}

macro_rules! until {
    ($sq:expr, $cq:expr, $expr:expr) => {{
        let deadline = Instant::now() + BUDGET;
        loop {
            match $expr {
                Stream::Next(Ok(v)) => break v,
                Stream::Next(Err(e)) => panic!("{e}"),
                _ => {}
            }
            assert!(Instant::now() < deadline, "timed out driving HTTP/3");
            pump(&mut $sq, &mut $cq);
        }
    }};
}

#[test]
fn the_driver_wakes_accept_waiters_and_parked_requests() {
    let server_cfg = server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config");
    let client_cfg = client_config_trusting_pem(CA_PEM).expect("client config");
    let mut sq = QuicDriver::server("127.0.0.1:0".parse().unwrap(), server_cfg).expect("bind");
    let addr = sq.local_addr().expect("addr");

    // Parked before any connection exists: woken when one is handshaked.
    let accept = Arc::new(CountingWaker::default());
    sq.accept_wakers()
        .lock()
        .unwrap()
        .push(Waker::from(Arc::clone(&accept)));

    let (mut cq, client_conn) = QuicDriver::connect(addr, client_cfg, "localhost").expect("connect");
    let deadline = Instant::now() + BUDGET;
    let server_conn = loop {
        if let Some(conn) = sq.take_accepted() {
            break conn;
        }
        assert!(Instant::now() < deadline, "server never accepted");
        pump(&mut sq, &mut cq);
    };
    assert!(accept.0.load(Ordering::SeqCst) >= 1, "the accept waiter was never woken");

    let mut server = H3Connection::new(server_conn);
    let mut client = H3Connection::new(client_conn);
    until!(sq, cq, server.poll_setup());
    until!(sq, cq, client.poll_setup());
    until!(sq, cq, client.poll_peer_settings());
    until!(sq, cq, server.poll_peer_settings());

    let mut req: H3Request<QuinnBidiStream> = until!(sq, cq, client.poll_open_request());
    let fields: Vec<(&[u8], &[u8])> = vec![
        (b":method", b"POST"),
        (b":scheme", b"https"),
        (b":authority", b"example.com"),
        (b":path", b"/park"),
    ];
    let mut headers = H3Request::<QuinnBidiStream>::encode_headers(&fields);
    until!(sq, cq, req.poll_send_headers(&mut headers));
    let mut inbound = until!(sq, cq, server.poll_accept());
    until!(sq, cq, inbound.poll_headers());

    // The server request has no body yet: park on it, as a future would.
    let parked = Arc::new(CountingWaker::default());
    assert!(
        inbound.register_waker(&Waker::from(Arc::clone(&parked))),
        "quinn connections can wake parked futures"
    );
    assert!(!matches!(inbound.poll_body(), Stream::Next(_)), "no body yet");
    assert_eq!(parked.0.load(Ordering::SeqCst), 0);

    // The body arrives: the driver wakes the parked request.
    let mut data = H3Request::<QuinnBidiStream>::encode_data(Bytes::from_static(b"wake"));
    until!(sq, cq, req.poll_send_data(&mut data));
    let deadline = Instant::now() + BUDGET;
    while parked.0.load(Ordering::SeqCst) == 0 {
        assert!(Instant::now() < deadline, "the parked request was never woken");
        pump(&mut sq, &mut cq);
    }
    let chunk = until!(sq, cq, inbound.poll_body());
    assert_eq!(chunk.as_deref(), Some(&b"wake"[..]));
}
