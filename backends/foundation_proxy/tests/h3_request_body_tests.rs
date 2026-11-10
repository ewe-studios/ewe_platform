//! The H3 proxy streams the request body and parks while a request is idle
//! (spec 70 feature 72, E1).
//!
//! WHY: `h3_proxy::read_body` collected the whole client upload into one `Vec`
//! before it forwarded (rule 4), and `read_headers` / `read_body` polled the
//! stream on a 1 ms timer, so a client that held a request open used CPU all the
//! time.
//!
//! WHAT: a real QUIC client uploads 16 MiB through the proxy to an HTTP/1.1
//! upstream that hashes what it receives. The proxy's `BufferedBytesGauge` must
//! peak under 1 MiB, the upstream's SHA-256 must match, and a body over
//! `max_request_body` must get a 413. A second test opens a request, sends
//! nothing for one second and counts how often the relay polls its stream
//! (the process CPU is printed too, but it holds the QUIC endpoints' own idle cost,
//! so the test does not assert on it).

#![cfg(feature = "quic")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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

/// The size of the upload.
const BODY_LEN: usize = 16 * 1024 * 1024;
/// The size of one DATA frame the client sends.
const FRAME_LEN: usize = 64 * 1024;
/// The most request bytes the proxy may hold at once.
const PEAK_LIMIT: usize = 1024 * 1024;
/// The most times the relay may poll an idle request's stream in one second. A
/// waker-parked relay polls when the QUIC connection moves (keep-alive and ACK
/// traffic); a 1 ms timer would poll 1000 times.
const IDLE_POLL_LIMIT: usize = 100;
/// The hard time limit of a test.
const TIME_LIMIT: Duration = Duration::from_secs(120);

/// The tests of this file run one at a time: the CPU test measures the whole
/// process, so no other test may run beside it.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn frame_bytes(state: &mut u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_LEN);
    while out.len() < FRAME_LEN {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Read one HTTP/1.1 request from `sock`, hash its body (Content-Length or
/// chunked) and answer with the hex SHA-256 and the byte count.
fn serve_hashing(mut sock: TcpStream) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if sock.read(&mut byte).unwrap_or(0) == 0 {
            return;
        }
        head.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&head).to_ascii_lowercase();
    let mut hasher = Sha256::new();
    let mut total = 0usize;
    if text.contains("transfer-encoding: chunked") {
        loop {
            let mut line = Vec::new();
            while !line.ends_with(b"\r\n") {
                if sock.read(&mut byte).unwrap_or(0) == 0 {
                    return;
                }
                line.push(byte[0]);
            }
            let size =
                usize::from_str_radix(String::from_utf8_lossy(&line).trim(), 16).unwrap_or(0);
            if size == 0 {
                let mut tail = [0u8; 2];
                let _ = sock.read_exact(&mut tail);
                break;
            }
            let mut data = vec![0u8; size];
            if sock.read_exact(&mut data).is_err() {
                return;
            }
            hasher.update(&data);
            total += size;
            let mut crlf = [0u8; 2];
            let _ = sock.read_exact(&mut crlf);
        }
    } else if let Some(len) = text
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        let mut left = len;
        let mut buf = vec![0u8; 64 * 1024];
        while left > 0 {
            let n = sock.read(&mut buf[..left.min(64 * 1024)]).unwrap_or(0);
            if n == 0 {
                return;
            }
            hasher.update(&buf[..n]);
            left -= n;
            total += n;
        }
    }
    let digest: [u8; 32] = hasher.finalize().into();
    let body = format!("{total} {}", hex(&digest));
    let _ = sock.write_all(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
}

fn start_upstream() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind upstream");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        while let Ok((sock, _)) = listener.accept() {
            std::thread::spawn(move || serve_hashing(sock));
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
) -> bool {
    loop {
        let step = if headers {
            request.poll_send_headers(&mut pending)
        } else {
            request.poll_send_data(&mut pending)
        };
        match step {
            Stream::Next(Ok(())) => return true,
            Stream::Next(Err(_)) => return false,
            _ => {
                assert!(Instant::now() < deadline, "client send timed out");
                std::thread::sleep(ms(1));
            }
        }
    }
}

fn finish(request: &mut H3Request<QuinnBidiStream>, deadline: Instant) {
    loop {
        match request.poll_finish() {
            Stream::Next(_) => return,
            _ => {
                assert!(Instant::now() < deadline, "finish timed out");
                std::thread::sleep(ms(1));
            }
        }
    }
}

struct Fixture {
    proxy: ProxyServer,
    request: H3Request<QuinnBidiStream>,
    /// The encoded request HEADERS frame.
    head: Bytes,
    deadline: Instant,
    dir: std::path::PathBuf,
}

/// Start the proxy with `max_request_body` and open one POST request stream.
fn open_request(tag: &str, max_request_body: usize, path: &str, send_head: bool) -> Fixture {
    let upstream_port = start_upstream();
    let dir = std::env::temp_dir().join(format!("ewe_h3_up_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    std::fs::write(&cert_path, CERT).expect("cert");
    std::fs::write(&key_path, KEY).expect("key");
    let svc =
        ServiceConfig::new("up", "localhost").backend(&format!("http://127.0.0.1:{upstream_port}"));
    let config = ProxyConfig::new("localhost", "127.0.0.1")
        .bind("127.0.0.1:0")
        .ssl(SslConfig::static_cert(
            &cert_path.display().to_string(),
            &key_path.display().to_string(),
        ))
        .h3_bind("127.0.0.1:0")
        .max_request_body(max_request_body)
        .service(svc);
    let proxy = ProxyServer::start(config, foundation_context::Ctx::host())
        .block()
        .expect("start proxy");
    let h3_addr = proxy.h3_local_addr().expect("H3 front end bound");
    std::thread::sleep(ms(300));

    let deadline = Instant::now() + TIME_LIMIT;
    let client_cfg = client_config_trusting_pem(CA).expect("client config");
    let (driver, conn) = QuicDriver::connect(h3_addr, client_cfg, "localhost").expect("connect");
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
        (b":method".as_ref(), b"POST".as_ref()),
        (b":scheme".as_ref(), b"https".as_ref()),
        (b":authority".as_ref(), b"localhost".as_ref()),
        (b":path".as_ref(), path.as_bytes()),
    ]);
    if send_head {
        send_frame(&mut request, head.clone(), true, deadline);
    }
    // Keep the connection alive for the life of the fixture.
    std::mem::forget(h3);
    Fixture {
        proxy,
        request,
        head,
        deadline,
        dir,
    }
}

/// Read the response status and body.
fn read_response(request: &mut H3Request<QuinnBidiStream>, deadline: Instant) -> (String, String) {
    let headers = drive(|| request.poll_headers(), deadline).expect("response headers");
    let status = headers
        .iter()
        .find(|(k, _)| k.as_ref() == b":status")
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .expect("status");
    let mut body = Vec::new();
    loop {
        match request.poll_body() {
            Stream::Next(Ok(Some(chunk))) => body.extend_from_slice(&chunk),
            Stream::Next(_) => break,
            _ => {
                assert!(Instant::now() < deadline, "response body timed out");
                std::thread::sleep(ms(1));
            }
        }
    }
    (status, String::from_utf8_lossy(&body).to_string())
}

#[test]
fn h3_request_body_is_streamed() {
    let _serial = one_at_a_time();
    let _guard = initialize_pool(93, Some(8));
    let mut fx = open_request("stream", 0, "/up", true);

    let mut hasher = Sha256::new();
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    for _ in 0..(BODY_LEN / FRAME_LEN) {
        let frame = frame_bytes(&mut state);
        hasher.update(&frame);
        let data = H3Request::<QuinnBidiStream>::encode_data(Bytes::from(frame));
        assert!(
            send_frame(&mut fx.request, data, false, fx.deadline),
            "upload send failed"
        );
    }
    finish(&mut fx.request, fx.deadline);

    let (status, body) = read_response(&mut fx.request, fx.deadline);
    let peak = fx.proxy.state().h3_body_gauge().peak();
    let held_at_end = fx.proxy.state().h3_body_gauge().current();
    eprintln!("peak request bytes held by the proxy: {peak}");
    fx.proxy.shutdown();
    let _ = std::fs::remove_dir_all(&fx.dir);

    let digest: [u8; 32] = hasher.finalize().into();
    assert_eq!(status, "200");
    assert_eq!(
        body,
        format!("{BODY_LEN} {}", hex(&digest)),
        "upstream saw the whole upload"
    );
    assert_eq!(held_at_end, 0, "the proxy let go of every byte it held");
    assert!(
        peak < PEAK_LIMIT,
        "the proxy held {peak} request bytes at once; the limit is {PEAK_LIMIT}"
    );
}

#[test]
fn h3_request_body_over_the_limit_gets_413() {
    let _serial = one_at_a_time();
    let _guard = initialize_pool(94, Some(8));
    let mut fx = open_request("limit", 256 * 1024, "/up", true);
    let mut state = 0x1234_5678_9ABC_DEF1_u64;
    // 1 MiB against a 256 KiB limit. The send may fail once the proxy resets the
    // request stream; the response is what matters.
    for _ in 0..16 {
        let data = H3Request::<QuinnBidiStream>::encode_data(Bytes::from(frame_bytes(&mut state)));
        if !send_frame(&mut fx.request, data, false, fx.deadline) {
            break;
        }
    }
    let (status, _) = read_response(&mut fx.request, fx.deadline);
    fx.proxy.shutdown();
    let _ = std::fs::remove_dir_all(&fx.dir);
    assert_eq!(status, "413");
}

/// The CPU time this process has used, all threads.
fn process_cpu() -> Duration {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid out pointer for the call.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime");
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

/// Hold still for one second and return the process CPU used, with the busiest
/// threads' tick counts.
fn measure_idle() -> (Duration, Vec<(String, u64)>) {
    let before = process_cpu();
    let ticks_before = thread_ticks();
    std::thread::sleep(Duration::from_secs(1));
    let used = process_cpu() - before;
    let mut threads = Vec::new();
    for (tid, (name, ticks)) in thread_ticks() {
        let was = ticks_before.get(&tid).map_or(0, |t| t.1);
        if ticks > was {
            threads.push((format!("{tid} {name}"), ticks - was));
        }
    }
    (used, threads)
}

/// Per-thread CPU ticks (user + system) from `/proc`, keyed by thread id.
fn thread_ticks() -> std::collections::HashMap<String, (String, u64)> {
    let mut out = std::collections::HashMap::new();
    for entry in std::fs::read_dir("/proc/self/task")
        .expect("task dir")
        .flatten()
    {
        let tid = entry.file_name().to_string_lossy().to_string();
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else {
            continue;
        };
        let name = stat[open + 1..close].to_string();
        let fields: Vec<&str> = stat[close + 2..].split_whitespace().collect();
        let ticks = fields[11].parse::<u64>().unwrap_or(0) + fields[12].parse::<u64>().unwrap_or(0);
        out.insert(tid, (name, ticks));
    }
    out
}

#[test]
fn h3_idle_request_uses_no_cpu() {
    let _serial = one_at_a_time();
    let _guard = initialize_pool(95, Some(8));
    // The connection is open and the request stream exists, but no HEADERS yet.
    let mut fx = open_request("idle", 0, "/up", false);
    std::thread::sleep(ms(500));
    let (connection_only, _) = measure_idle();

    // Now the request head arrives: the handler starts and waits for the body.
    let head = fx.head.clone();
    assert!(send_frame(&mut fx.request, head, true, fx.deadline));
    std::thread::sleep(ms(500));
    let polls_before = fx.proxy.state().h3_body_gauge().polls();
    let (with_request, per_thread) = measure_idle();
    let polls = fx.proxy.state().h3_body_gauge().polls() - polls_before;
    for (name, ticks) in per_thread {
        eprintln!("  thread {name}: {ticks} ticks");
    }
    let cpu_added = with_request.saturating_sub(connection_only);
    eprintln!(
        "idle connection {connection_only:?}, with an open request {with_request:?} \
         (added {cpu_added:?}); stream polls in the idle second: {polls}"
    );

    // The request still works afterwards.
    finish(&mut fx.request, fx.deadline);
    let (status, _) = read_response(&mut fx.request, fx.deadline);
    fx.proxy.shutdown();
    let _ = std::fs::remove_dir_all(&fx.dir);
    assert_eq!(status, "200");
    // The relay parks on the connection's waker: it polls the stream when the
    // connection makes progress, not on a timer (a 1 ms timer is 1000 polls).
    assert!(
        polls < IDLE_POLL_LIMIT,
        "an open idle request polled its stream {polls} times in one second; the limit is {IDLE_POLL_LIMIT}"
    );
}
