// Socket and address constants are small and fit the libc types.
#![allow(clippy::cast_possible_truncation)]

//! The `io_uring` upstream dial does not hold a worker (spec 70 feature 70, E2).
//!
//! WHY: `aio::dial_tcp` used `foundation_iogate::connect_completion` for the
//! `io_uring` modes, which called `TcpStream::connect`. A dial to an address that
//! never answers then blocked the worker thread for the whole kernel connect
//! timeout (minutes), and on a one-worker pool no other connection was served.
//!
//! WHAT: a backend whose accept queue is full drops every new SYN, so a dial to
//! it hangs. A passthrough with a two second connect timeout dials it while a
//! second passthrough, on the same small pool, must still echo a client.
//!
//! The pool has two task workers at least (`initialize_pool` raises a smaller
//! request to three threads, two of them task workers), so the test opens more
//! stuck dials than there are workers: with a blocking dial every worker is held.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::fd::FromRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{sleep_async, valtron_test};
use foundation_iogate::{init_reactor_for, ServerIo};
use foundation_proxy::passthrough::PassthroughConfig;
use foundation_proxy::TcpPassthrough;

/// The connect timeout the stuck passthrough runs with.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// How many dials to the blackhole run at once: more than the pool has task workers.
const STUCK_DIALS: usize = 6;

/// A listener with a full accept queue: it never accepts, so the kernel drops
/// the SYN of any further connect and the connecting side waits.
struct Blackhole {
    addr: SocketAddr,
    _listener: TcpListener,
    _fillers: Vec<TcpStream>,
}

impl Blackhole {
    fn start() -> Self {
        // SAFETY: plain socket calls on an fd this function owns; the fd moves
        // into the `TcpListener`, which closes it.
        let listener = unsafe {
            let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
            assert!(fd >= 0, "socket");
            let addr = libc::sockaddr_in {
                // BSD sockets carry the struct length in the address itself.
                #[cfg(any(
                    target_os = "macos",
                    target_os = "ios",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd"
                ))]
                sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
                sin_family: libc::AF_INET as libc::sa_family_t,
                sin_port: 0,
                sin_addr: libc::in_addr {
                    s_addr: u32::from_ne_bytes([127, 0, 0, 1]),
                },
                sin_zero: [0; 8],
            };
            let size = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
            assert_eq!(libc::bind(fd, (&raw const addr).cast(), size), 0, "bind");
            assert_eq!(libc::listen(fd, 0), 0, "listen");
            TcpListener::from_raw_fd(fd)
        };
        let addr = listener.local_addr().expect("addr");
        // Fill the queue: connect until one attempt gets no answer.
        let mut fillers = Vec::new();
        loop {
            match TcpStream::connect_timeout(&addr, Duration::from_millis(300)) {
                Ok(s) => fillers.push(s),
                Err(_) => break,
            }
            assert!(fillers.len() < 16, "the accept queue never filled");
        }
        Self {
            addr,
            _listener: listener,
            _fillers: fillers,
        }
    }
}

/// A one-thread echo server.
fn start_echo() -> (u16, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind echo");
    listener.set_nonblocking(true).expect("nonblocking");
    let port = listener.local_addr().expect("addr").port();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    std::thread::spawn(move || {
        while !flag.load(Ordering::SeqCst) {
            if let Ok((mut s, _)) = listener.accept() {
                std::thread::spawn(move || {
                    s.set_nonblocking(false).ok();
                    let mut buf = [0u8; 256];
                    while let Ok(n) = s.read(&mut buf) {
                        if n == 0 || s.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                });
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    });
    (port, stop)
}

/// Whether `io_uring` is missing, so the caller skips its test.
///
/// A local machine without `io_uring` skips with a message. An environment that
/// sets `EWE_REQUIRE_IO_URING=1` expects `io_uring`, so a missing `io_uring` fails the
/// test there: CI cannot lose this coverage without anyone noticing.
fn io_uring_missing(what: &str) -> bool {
    if init_reactor_for(ServerIo::Completion).is_ok() {
        return false;
    }
    assert!(
        std::env::var("EWE_REQUIRE_IO_URING").as_deref() != Ok("1"),
        "EWE_REQUIRE_IO_URING=1 but io_uring is unavailable ({what})"
    );
    eprintln!("io_uring unavailable — skipping {what}");
    true
}

#[valtron_test(seed = 70, threads = 1, timeout = 60000)]
async fn completion_dial_does_not_hold_worker() {
    if io_uring_missing("completion dial test") {
        return;
    }
    let hole = Blackhole::start();
    let (echo_port, echo_stop) = start_echo();

    let config = PassthroughConfig::new()
        .with_io_mode(ServerIo::Completion)
        .with_connect_timeout(CONNECT_TIMEOUT);
    let stuck = TcpPassthrough::start_with(
        "127.0.0.1:0",
        &hole.addr.to_string(),
        config,
        foundation_core::valtron::Ctx::host(),
    )
    .expect("start stuck passthrough");
    let good = TcpPassthrough::start_with(
        "127.0.0.1:0",
        &format!("127.0.0.1:{echo_port}"),
        config,
        foundation_core::valtron::Ctx::host(),
    )
    .expect("start good passthrough");

    // These clients make the proxy dial the blackhole: more dials than workers.
    let stuck_clients: Vec<TcpStream> = (0..STUCK_DIALS)
        .map(|_| TcpStream::connect(stuck.local_addr()).expect("connect stuck"))
        .collect();
    sleep_async(
        Duration::from_millis(500),
        &foundation_core::valtron::Ctx::host(),
    )
    .await;

    // While that dial is open, a second client must be answered.
    let good_addr = good.local_addr();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut c = TcpStream::connect(good_addr).expect("connect good");
        c.set_read_timeout(Some(Duration::from_secs(10))).ok();
        c.write_all(b"ping").expect("write");
        let mut buf = [0u8; 8];
        let n = c.read(&mut buf).unwrap_or(0);
        let _ = tx.send((buf[..n].to_vec(), started.elapsed()));
    });
    let answer = loop {
        if let Ok(v) = rx.try_recv() {
            break v;
        }
        sleep_async(
            Duration::from_millis(20),
            &foundation_core::valtron::Ctx::host(),
        )
        .await;
    };
    assert_eq!(answer.0, b"ping", "the second request was not echoed");
    assert!(
        answer.1 < Duration::from_millis(1500),
        "the second request waited {:?} behind the open dial",
        answer.1
    );

    // The dials give up at the connect timeout: the proxy then drops each stuck
    // client, which sees the end of its stream.
    let started = Instant::now();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut closed = 0;
        for mut c in stuck_clients {
            c.set_read_timeout(Some(Duration::from_secs(10))).ok();
            let mut buf = [0u8; 1];
            // `Ok(0)` is the orderly close, `Err` a reset; a read timeout is a miss.
            match c.read(&mut buf) {
                Ok(0) => closed += 1,
                Err(e)
                    if e.kind() != std::io::ErrorKind::WouldBlock
                        && e.kind() != std::io::ErrorKind::TimedOut =>
                {
                    closed += 1;
                }
                _ => {}
            }
        }
        let _ = tx.send((closed, started.elapsed()));
    });
    let (closed, took) = loop {
        if let Ok(v) = rx.try_recv() {
            break v;
        }
        sleep_async(
            Duration::from_millis(20),
            &foundation_core::valtron::Ctx::host(),
        )
        .await;
    };
    assert_eq!(
        closed, STUCK_DIALS,
        "every stuck dial must end at the connect timeout"
    );
    assert!(
        took < CONNECT_TIMEOUT + Duration::from_secs(3),
        "the stuck dials took {took:?}, the timeout is {CONNECT_TIMEOUT:?}"
    );
    echo_stop.store(true, Ordering::SeqCst);
    stuck.shutdown();
    good.shutdown();
}
