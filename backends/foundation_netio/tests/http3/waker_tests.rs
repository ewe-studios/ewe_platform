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

use foundation_core::valtron::{Ctx, Stream, TaskIterator, TaskStatus};
use foundation_netio::http3::connection::{H3Connection, H3Request};
use foundation_netio::quic::{
    client_config_trusting_pem, server_config_from_pem, QuicDriver, QuicEvent, QuinnBidiStream,
};
#[cfg(unix)]
use foundation_reactor::FdReadiness;

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

/// Wraps a [`QuicDriver`] and counts every `next_status` call. The idle test
/// asserts the count stays tiny, which proves the driver parks rather than
/// polling at 1 ms — a deterministic proxy for CPU, with no process-wide clock.
struct CountingDriver {
    inner: QuicDriver,
    polls: Arc<AtomicUsize>,
}

impl CountingDriver {
    fn new(inner: QuicDriver) -> (Self, Arc<AtomicUsize>) {
        let polls = Arc::new(AtomicUsize::new(0));
        (Self { inner, polls: polls.clone() }, polls)
    }
}

impl TaskIterator for CountingDriver {
    type Ready = QuicEvent;
    type Pending = ();

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<QuicEvent, ()>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        self.inner.next_status(ctx)
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

/// An idle HTTP/3 connection uses no CPU: the driver parks on its socket's fd
/// instead of busy-polling at 1 ms. The poll count over one idle second is the
/// deterministic proof — a polling driver calls `next_status` ~1000 times, a
/// parked one stays at ~0.
#[cfg(all(feature = "multi", unix))]
#[test]
fn idle_h3_connection_uses_no_cpu() {
    use std::os::fd::AsRawFd;
    use foundation_core::valtron::{initialize_pool, send};

    let _guard = initialize_pool(200, Some(4));

    let server_cfg = server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config");
    let sq = QuicDriver::server("127.0.0.1:0".parse().unwrap(), server_cfg).expect("bind");
    let addr = sq.local_addr().expect("addr");
    let accept_queue = sq.accept_queue();
    let fd = FdReadiness::register(sq.socket().as_raw_fd()).expect("register fd");
    let (counting, polls) = CountingDriver::new(sq.with_fd_readiness(Arc::new(fd)));
    send(counting).expect("spawn server driver");

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

    // Let it settle into its parked state, then count polls over one idle
    // second. A driver that parks on the socket stays at ~0; one that polls at
    // 1 ms would fire ~1000 times.
    std::thread::sleep(Duration::from_millis(100));
    polls.store(0, Ordering::SeqCst);
    std::thread::sleep(Duration::from_secs(1));
    let count = polls.load(Ordering::SeqCst);
    assert!(
        count < 20,
        "idle h3 driver polled {count} times in 1 s; it should park on the socket"
    );
}

/// The driver parks on the fd when one is injected, and falls back to a 1 ms
/// poll when it is not. The direct, deterministic form of the CPU test: with no
/// traffic the idle driver returns `Depends`, never `Delayed`.
#[cfg(unix)]
#[test]
fn driver_parks_on_fd_readiness_not_delayed() {
    use std::os::fd::AsRawFd;

    // No fd readiness: an idle server driver falls back to the 1 ms poll.
    let mut no_fd = QuicDriver::server(
        "127.0.0.1:0".parse().unwrap(),
        server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config"),
    )
    .expect("bind");
    assert!(
        matches!(no_fd.next_status(Ctx::new()), Some(TaskStatus::Delayed(_))),
        "without fd readiness the idle driver should poll, not park"
    );

    // With fd readiness: it parks on the reactor.
    let sq = QuicDriver::server(
        "127.0.0.1:0".parse().unwrap(),
        server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config"),
    )
    .expect("bind");
    let fd = FdReadiness::register(sq.socket().as_raw_fd()).expect("register fd");
    let mut sq = sq.with_fd_readiness(Arc::new(fd));
    assert!(
        matches!(sq.next_status(Ctx::new()), Some(TaskStatus::Depends(_))),
        "with fd readiness the idle driver should park, not poll"
    );
}
