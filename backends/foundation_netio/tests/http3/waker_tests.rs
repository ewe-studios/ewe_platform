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

/// Process CPU time (user + system) in nanoseconds, via `getrusage`.
#[cfg(all(feature = "multi", unix))]
fn process_cpu_ns() -> u128 {
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        let rc = libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        assert_eq!(rc, 0, "getrusage failed");
        let user = usage.ru_utime.tv_sec as u128 * 1_000_000_000
            + usage.ru_utime.tv_usec as u128 * 1_000;
        let system = usage.ru_stime.tv_sec as u128 * 1_000_000_000
            + usage.ru_stime.tv_usec as u128 * 1_000;
        user + system
    }
}

/// An idle HTTP/3 connection uses no CPU: the request futures park on the
/// connection instead of busy-polling. The driver's own 1 ms idle poll is the
/// only remaining cost, and it stays well under the 50 ms/s budget.
#[cfg(all(feature = "multi", unix))]
#[test]
fn idle_h3_connection_uses_no_cpu() {
    use foundation_core::valtron::{initialize_pool, send};

    let _guard = initialize_pool(200, Some(4));

    let server_cfg = server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config");
    let sq = QuicDriver::server("127.0.0.1:0".parse().unwrap(), server_cfg).expect("bind");
    let addr = sq.local_addr().expect("addr");
    let accept_queue = sq.accept_queue();
    send(sq).expect("spawn server driver");

    let client_cfg = client_config_trusting_pem(CA_PEM).expect("client config");
    let (mut cq, client_conn) = QuicDriver::connect(addr, client_cfg, "localhost").expect("connect");

    // Drive the client by hand while the server driver runs on the pool.
    let deadline = Instant::now() + BUDGET;
    let server_conn = loop {
        let _ = cq.next_status(Ctx::new());
        if let Some(conn) = accept_queue.lock().unwrap().pop_front() {
            break conn;
        }
        assert!(Instant::now() < deadline, "server never accepted");
        std::thread::sleep(Duration::from_millis(1));
    };

    // Finish H3 setup on both sides so the connection is fully established and
    // idle (no request stream open).
    let mut server = H3Connection::new(server_conn);
    let mut client = H3Connection::new(client_conn);
    let setup_deadline = Instant::now() + BUDGET;
    loop {
        let _ = cq.next_status(Ctx::new());
        let server_ready = matches!(server.poll_setup(), Stream::Next(Ok(())));
        let client_ready = matches!(client.poll_setup(), Stream::Next(Ok(())));
        if server_ready && client_ready {
            break;
        }
        assert!(Instant::now() < setup_deadline, "h3 setup timed out");
        std::thread::sleep(Duration::from_millis(1));
    }

    // Let it settle into its parked state, then measure a full idle second.
    std::thread::sleep(Duration::from_millis(100));
    let before = process_cpu_ns();
    std::thread::sleep(Duration::from_secs(1));
    let after = process_cpu_ns();
    let cpu_ms = (after - before) / 1_000_000;
    assert!(
        cpu_ms < 50,
        "idle h3 connection used {cpu_ms} ms of CPU in 1 s; it should park"
    );
}
