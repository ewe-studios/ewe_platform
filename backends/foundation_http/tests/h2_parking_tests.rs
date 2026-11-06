//! An idle HTTP/2 connection task parks on its socket instead of polling.
//!
//! `H2ConnectionHandler` used to re-poll every 10 ms (1 ms during the
//! handshake) whenever no frame was buffered — the "47b" reactor-parking gap.
//! It now registers the socket with the shared reactor and returns
//! `TaskStatus::Depends`, resuming when the socket, a handler's response pipe,
//! the idle deadline or shutdown says so.

#![cfg(all(unix, not(target_family = "wasm")))]

#[cfg(feature = "multi")]
use std::io::Read;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "multi")]
use bytes::Bytes;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::WaitGroup;
#[cfg(feature = "multi")]
use foundation_core::valtron::initialize_pool;
use foundation_core::valtron::{Ctx, TaskIterator, TaskStatus};
use foundation_http::native::server::H2ConnectionHandler;
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
#[cfg(feature = "multi")]
use foundation_netio::http2::channel::H2Channel;
use foundation_netio::http2::conn::H2Conn;
#[cfg(feature = "multi")]
use foundation_netio::http2::connection::H2Request;
use foundation_netio::http2::connection::CLIENT_PREFACE;
use foundation_netio::netcap::{ConnectionContext, RawStream};

#[test]
fn an_idle_h2_connection_parks_on_its_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let mut client = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
    let (server, _) = listener.accept().expect("accept");
    server.set_nonblocking(true).expect("non-blocking");

    // The client preface and an empty SETTINGS frame: the whole handshake.
    client.write_all(CLIENT_PREFACE).expect("preface");
    client
        .write_all(&[0, 0, 0, 0x4, 0, 0, 0, 0, 0])
        .expect("settings");

    let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).expect("raw stream"));
    let waits = WaitGroup::new();
    waits.add(1);
    let mut handler = H2ConnectionHandler::new(
        H2Conn::new_server(stream),
        Arc::new(HttpApp::new_h2_serve()),
        Arc::new(ConnectionContext::default()),
        Arc::new(ShutdownSignal::new()),
        waits.guard(),
        Arc::new(foundation_core::valtron::MonotonicClock::new()),
    );

    let mut last = None;
    for _ in 0..50 {
        let status = handler
            .next_status(Ctx::new())
            .expect("the connection stays open");
        let parked = matches!(status, TaskStatus::Depends(_));
        assert!(
            !matches!(status, TaskStatus::Delayed(_)),
            "the handler polled on a timer instead of parking"
        );
        last = Some(parked);
        if parked {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(last, Some(true), "an idle connection must park (Depends)");
    drop(client);
}

// ── CPU regression tests (real pool + reactor) ──────────────────────────

/// Per-thread CPU time, keyed by the operating system's thread id.
///
/// WHY: the idle CPU test used to measure the whole process (`getrusage`), so a
/// thread of another test in the same binary counted against the connection.
/// A per-thread measure names the threads that use CPU and cannot be disturbed
/// by a parallel test.
#[cfg(all(feature = "multi", any(target_os = "linux", target_os = "macos")))]
mod thread_cpu {
    use std::collections::BTreeMap;

    /// Map a stable thread id to its CPU nanoseconds and name.
    pub fn snapshot() -> BTreeMap<u64, (u64, String)> {
        imp::snapshot()
    }

    #[cfg(target_os = "linux")]
    mod imp {
        use super::BTreeMap;

        pub fn snapshot() -> BTreeMap<u64, (u64, String)> {
            let mut out = BTreeMap::new();
            for entry in std::fs::read_dir("/proc/self/task")
                .expect("task dir")
                .flatten()
            {
                let Ok(tid) = entry.file_name().to_string_lossy().parse::<u64>() else {
                    continue;
                };
                let cpu = std::fs::read_to_string(entry.path().join("schedstat"))
                    .ok()
                    .and_then(|s| {
                        s.split_whitespace()
                            .next()
                            .and_then(|v| v.parse::<u64>().ok())
                    })
                    .unwrap_or(0);
                let name = std::fs::read_to_string(entry.path().join("comm"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                out.insert(tid, (cpu, name));
            }
            out
        }
    }

    #[cfg(target_os = "macos")]
    mod imp {
        use super::BTreeMap;
        use std::mem::size_of;

        /// `PROC_PIDLISTTHREADS` from `<sys/proc_info.h>`. The libc crate does
        /// not export this constant.
        const PROC_PIDLISTTHREADS: libc::c_int = 6;

        pub fn snapshot() -> BTreeMap<u64, (u64, String)> {
            let mut out = BTreeMap::new();
            let pid = libc::pid_t::try_from(std::process::id()).unwrap_or(0);
            let mut tids = vec![0u64; 512];
            // SAFETY: `tids` is a writable buffer of the size passed to the call.
            let bytes = unsafe {
                libc::proc_pidinfo(
                    pid,
                    PROC_PIDLISTTHREADS,
                    0,
                    tids.as_mut_ptr().cast(),
                    i32::try_from(tids.len() * size_of::<u64>()).unwrap_or(i32::MAX),
                )
            };
            if bytes <= 0 {
                return out;
            }
            let count = usize::try_from(bytes).unwrap_or(0) / size_of::<u64>();
            for tid in tids.into_iter().take(count) {
                let mut info = std::mem::MaybeUninit::<libc::proc_threadinfo>::zeroed();
                let info_size = i32::try_from(size_of::<libc::proc_threadinfo>()).unwrap_or(0);
                // SAFETY: `info` is a writable buffer of `info_size` bytes.
                let got = unsafe {
                    libc::proc_pidinfo(
                        pid,
                        libc::PROC_PIDTHREADINFO,
                        tid,
                        info.as_mut_ptr().cast(),
                        info_size,
                    )
                };
                if got != info_size {
                    continue;
                }
                // SAFETY: the call filled the whole `proc_threadinfo`.
                let info = unsafe { info.assume_init() };
                let name: Vec<u8> = info
                    .pth_name
                    .iter()
                    .take_while(|&&c| c != 0)
                    .map(|&c| u8::try_from(c).unwrap_or(b'?'))
                    .collect();
                let cpu = info.pth_user_time.saturating_add(info.pth_system_time);
                out.insert(tid, (cpu, String::from_utf8_lossy(&name).into_owned()));
            }
            out
        }
    }
}

/// Write any pending channel output, then read whatever the server sent back.
/// The client socket is blocking with a short read timeout, so `read` returns
/// at once when the server answers and otherwise yields after the timeout.
#[cfg(feature = "multi")]
fn pump_h2(stream: &mut TcpStream, chan: &mut H2Channel) {
    let out = chan.drain_output();
    if !out.is_empty() {
        stream.write_all(&out).expect("client write");
    }
    let mut buf = [0u8; 8192];
    match stream.read(&mut buf) {
        Ok(0) => {}
        Ok(n) => chan.feed_input(&buf[..n]),
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut => {}
        Err(e) => panic!("client read: {e}"),
    }
}

/// Drive the `H2Channel` client handshake to completion over `stream`.
#[cfg(feature = "multi")]
fn h2_client_handshake(stream: &mut TcpStream, chan: &mut H2Channel) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match chan.client_handshake_step() {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(std::time::Instant::now() < deadline, "handshake timed out");
                pump_h2(stream, chan);
            }
            Err(e) => panic!("client handshake: {e}"),
        }
    }
}

/// Send one `GET /` request and wait until its response headers arrive.
#[cfg(feature = "multi")]
fn h2_client_get(stream: &mut TcpStream, chan: &mut H2Channel) {
    chan.send_request(&H2Request {
        method: Bytes::from_static(b"GET"),
        scheme: Bytes::from_static(b"http"),
        authority: Bytes::from_static(b"test"),
        path: Bytes::from_static(b"/"),
        headers: Vec::new(),
        body: None,
        end_stream: true,
    })
    .expect("send request");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        pump_h2(stream, chan);
        match chan.recv_response() {
            Ok(Some(_)) => return,
            Ok(None) => panic!("server sent GOAWAY"),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("recv response: {e}"),
        }
        assert!(std::time::Instant::now() < deadline, "response timed out");
    }
}

/// Start one H2 connection handler on the pool with a real loopback socket,
/// returning the client end once the handshake and one request are done.
#[cfg(feature = "multi")]
fn idle_h2_client() -> (TcpStream, H2Channel) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let mut client = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
    let (server, _) = listener.accept().expect("accept");
    server.set_nonblocking(true).expect("non-blocking");
    client
        .set_read_timeout(Some(Duration::from_millis(200)))
        .expect("read timeout");

    let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).expect("raw stream"));
    let waits = WaitGroup::new();
    waits.add(1);
    let handler = H2ConnectionHandler::new(
        H2Conn::new_server(stream),
        Arc::new(HttpApp::new_h2_serve()),
        Arc::new(ConnectionContext::default()),
        Arc::new(ShutdownSignal::new()),
        waits.guard(),
        Arc::new(foundation_core::valtron::MonotonicClock::new()),
    );
    foundation_core::valtron::send(handler).expect("spawn h2 handler");

    let mut chan = H2Channel::new(false);
    h2_client_handshake(&mut client, &mut chan);
    h2_client_get(&mut client, &mut chan);
    (client, chan)
}

/// The CPU cap for one idle second, in milliseconds.
///
/// The connection parks on its socket, so the only wakes are the executor's
/// `max_yield_duration` (100 ms, `ExecutorConfig`) and the reactor drain timer.
/// Feature 77 measured 0.6 to 1.2 ms of CPU per idle valtron worker per second
/// on Linux; four workers plus the reactor drain thread are therefore about 1 to
/// 5 ms. The cap is 8 ms: strict enough to catch a 10 ms poll (100 wake-ups per
/// second), with headroom for a loaded CI host. The old process-wide cap was
/// 50 ms and counted every thread of the test binary.
#[cfg(all(feature = "multi", any(target_os = "linux", target_os = "macos")))]
const IDLE_CPU_CAP_MS: u128 = 8;

/// An idle HTTP/2 connection uses no CPU. It parks on its socket, so one second
/// of silence costs the server's own threads (the pool workers and the reactor
/// drain thread) under [`IDLE_CPU_CAP_MS`]. The old 10 ms timer woke it 100
/// times a second.
///
/// The threads are found by a snapshot before the pool starts: the workers and
/// the reactor thread are the ones that appear after it. A parallel test cannot
/// disturb the measure, because its threads are not in the set.
#[cfg(all(feature = "multi", any(target_os = "linux", target_os = "macos")))]
#[test]
fn idle_h2_connection_uses_no_cpu() {
    let before_pool = thread_cpu::snapshot();
    let _guard = initialize_pool(100, Some(4));
    let (client, _chan) = idle_h2_client();

    let at_start = thread_cpu::snapshot();
    let server: Vec<u64> = at_start
        .iter()
        .filter(|(key, (_, name))| {
            // The reactor is a process-wide singleton: another test may have
            // started its drain thread before the snapshot. Include it by name.
            !before_pool.contains_key(key) || name == "reactor-drain"
        })
        .map(|(key, _)| *key)
        .collect();
    assert!(
        !server.is_empty(),
        "the pool did not start a worker thread after the snapshot"
    );

    // Let the connection settle into its parked state before measuring.
    std::thread::sleep(Duration::from_millis(100));
    let before = thread_cpu::snapshot();
    std::thread::sleep(Duration::from_secs(1));
    let after = thread_cpu::snapshot();

    let mut total_ns = 0u64;
    let mut report = Vec::new();
    for key in &server {
        let (cpu0, name) = before.get(key).cloned().unwrap_or((0, String::new()));
        let cpu1 = after.get(key).map_or(cpu0, |(cpu, _)| *cpu);
        let used = cpu1.saturating_sub(cpu0);
        total_ns = total_ns.saturating_add(used);
        report.push(format!(
            "{}={} us",
            if name.is_empty() { "<unnamed>" } else { &name },
            used / 1_000
        ));
    }
    eprintln!("idle h2 server threads: {}", report.join(" "));

    let cpu_ms = u128::from(total_ns) / 1_000_000;
    assert!(
        cpu_ms < IDLE_CPU_CAP_MS,
        "idle h2 connection used {cpu_ms} ms of CPU in 1 s on the server's threads; \
         the cap is {IDLE_CPU_CAP_MS} ms: {}",
        report.join(" ")
    );
    drop(client);
}

/// After a second idle, a request is served at once — the socket wake works,
/// so the parked handler runs again as soon as bytes arrive instead of waiting
/// for a poll cycle.
#[cfg(feature = "multi")]
#[test]
fn h2_request_after_idle_is_served_at_once() {
    let _guard = initialize_pool(101, Some(4));
    let (mut client, mut chan) = idle_h2_client();

    std::thread::sleep(Duration::from_secs(1));

    let start = std::time::Instant::now();
    h2_client_get(&mut client, &mut chan);
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(200),
        "request after idle took {elapsed:?}; the socket wake did not fire at once"
    );
    drop(client);
}
