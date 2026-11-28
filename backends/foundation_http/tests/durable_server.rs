//! End-to-end tests for the Durable Streams HTTP surface.
//!
//! Each test starts a real `HttpServer` with the catch-all Durable handler over
//! the memory VFS, then drives it with the platform client.

#![cfg(all(feature = "multi", unix))]

use std::sync::Arc;
use std::time::Duration;

use foundation_core::durable::{DurableLimits, DurableStream, MemoryVfs};
use foundation_core::extensions::result_ext::BoxedError;
use foundation_core::valtron::initialize_pool;
use foundation_http::native::durable::{DurableHandler, DurableHttpConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::ShutdownSignal;
use foundation_http::{
    native::serve::Serve,
    native::server::{HttpServer, KeepAliveConfig, ServerConfig},
};
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::SimpleResponse;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, Status};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use serial_test::serial;

/// How long an idle keep-alive connection survives in these tests.
///
/// A raw-socket test has no retry: if the server closes the connection between
/// two requests, the next read fails. The tests also hold one connection across
/// many requests, so an idle timeout near a scheduling pause turns an unrelated
/// delay into a failure (issue #1127). Keep this longer than the longest test
/// watchdog (120 s in this file), so only the test's own `shutdown` can end an
/// idle connection while the test runs.
const TEST_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

fn start_server(app: HttpApp<Arc<dyn Serve>>) -> (std::net::SocketAddr, Arc<ShutdownSignal>) {
    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");

    let config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(TEST_IDLE_TIMEOUT));
    let bind_addr = format!("127.0.0.1:{}", addr.port());
    let server = HttpServer::with_config(app, &bind_addr, config);

    let shutdown_thread = shutdown.clone();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_thread);
    });

    (addr, shutdown)
}

fn make_client(addr: std::net::SocketAddr) -> DynNetClient {
    HttpClientBuilder::new()
        .resolve_to(addr)
        .max_retries(10)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(20))
        .write_timeout(Duration::from_secs(5))
        .build()
}

fn durable_app() -> HttpApp<Arc<dyn Serve>> {
    let ctx = Ctx::host();
    let _ = ctx.get_or_insert(DurableStream::new(Arc::new(MemoryVfs::new()), &ctx));
    let mut app = HttpApp::with_context(ctx);
    app.fallback::<DurableHandler>();
    app
}

fn durable_app_with_body_limit(max_body_bytes: usize) -> HttpApp<Arc<dyn Serve>> {
    let ctx = Ctx::host();
    let _ = ctx.get_or_insert(DurableStream::new(Arc::new(MemoryVfs::new()), &ctx));
    let _ = ctx.get_or_insert(DurableLimits::default().with_max_body_bytes(max_body_bytes));
    let mut app = HttpApp::with_context(ctx);
    app.fallback::<DurableHandler>();
    app
}

fn durable_app_with_http_config(
    limits: DurableLimits,
    config: DurableHttpConfig,
) -> HttpApp<Arc<dyn Serve>> {
    let ctx = Ctx::host();
    let _ = ctx.get_or_insert(DurableStream::new(Arc::new(MemoryVfs::new()), &ctx));
    let _ = ctx.get_or_insert(limits);
    let _ = ctx.get_or_insert(config);
    let mut app = HttpApp::with_context(ctx);
    app.fallback::<DurableHandler>();
    app
}

/// Read one HTTP/1.1 response head from a raw stream, then read and drop its
/// declared body. Return the status and the body length.
fn read_raw_head(stream: &mut std::net::TcpStream) -> (u16, usize) {
    use std::io::Read;

    let mut head = Vec::new();
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => head.push(byte[0]),
            Err(error) => panic!("read response head: {error}"),
        }
    }
    let head = String::from_utf8_lossy(&head);
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let content_length = head
        .lines()
        .find_map(|line| {
            let lower = line.to_ascii_lowercase();
            lower
                .strip_prefix("content-length:")
                .map(|value| value.trim().parse().unwrap_or(0))
        })
        .unwrap_or(0);
    let mut body = vec![0_u8; content_length];
    if content_length > 0 {
        stream.read_exact(&mut body).expect("read response body");
    }
    (status, content_length)
}

fn read_body(body: SendSafeBody) -> Result<String, BoxedError> {
    match body {
        SendSafeBody::Text(text) => Ok(text),
        SendSafeBody::Bytes(bytes) => Ok(String::from_utf8_lossy(&bytes).to_string()),
        body => {
            let bytes = try_collect_bytes(body)?;
            Ok(String::from_utf8_lossy(&bytes).to_string())
        }
    }
}

fn status_code(status: Status) -> u16 {
    u16::try_from(status.into_usize()).expect("a status code fits u16")
}

fn header(response: &SimpleResponse<SendSafeBody>, name: &SimpleHeader) -> Option<String> {
    let wanted = name.to_string();
    response
        .get_headers_ref()
        .iter()
        .find(|(key, _)| key.to_string().eq_ignore_ascii_case(&wanted))
        .and_then(|(_, values)| values.first().cloned())
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_a_durable_server_when_a_client_creates_appends_and_reads_then_it_round_trips() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = make_client(addr);

    let created = client
        .send(
            PreparedRequestBuilder::put("http://testserver/events")
                .unwrap()
                .header(SimpleHeader::CONTENT_TYPE, "text/plain")
                .build(),
        )
        .unwrap();
    assert_eq!(status_code(created.get_status()), 201);
    // The adapter makes the relative Location absolute for the request host,
    // which the reference conformance suite requires.
    assert_eq!(
        header(&created, &SimpleHeader::LOCATION).as_deref(),
        Some("http://testserver/events")
    );

    let appended = client
        .send(
            PreparedRequestBuilder::post("http://testserver/events")
                .unwrap()
                .body_text("hello")
                .build(),
        )
        .unwrap();
    assert_eq!(status_code(appended.get_status()), 204);
    let next = header(&appended, &SimpleHeader::custom("stream-next-offset")).expect("next offset");

    let read = client
        .send(
            PreparedRequestBuilder::get("http://testserver/events")
                .unwrap()
                .build(),
        )
        .unwrap();
    assert_eq!(status_code(read.get_status()), 200);
    assert_eq!(
        header(&read, &SimpleHeader::custom("stream-up-to-date")).as_deref(),
        Some("true")
    );

    let head = client
        .send(
            PreparedRequestBuilder::head("http://testserver/events")
                .unwrap()
                .build(),
        )
        .unwrap();
    assert_eq!(status_code(head.get_status()), 200);
    assert_eq!(
        header(&head, &SimpleHeader::custom("stream-next-offset")).as_deref(),
        Some(next.as_str())
    );

    let (_, _, body) = read.into_parts();
    assert_eq!(read_body(body).unwrap(), "hello");

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_a_json_stream_when_a_client_reads_it_then_it_gets_a_json_array() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = make_client(addr);

    let created = client
        .send(
            PreparedRequestBuilder::put("http://testserver/log")
                .unwrap()
                .header(SimpleHeader::CONTENT_TYPE, "application/json")
                .build(),
        )
        .unwrap();
    assert_eq!(status_code(created.get_status()), 201);

    for message in [r#"{"n":1}"#, r#"{"n":2}"#] {
        let appended = client
            .send(
                PreparedRequestBuilder::post("http://testserver/log")
                    .unwrap()
                    .header(SimpleHeader::CONTENT_TYPE, "application/json")
                    .body_text(message)
                    .build(),
            )
            .unwrap();
        assert_eq!(status_code(appended.get_status()), 204);
    }

    let read = client
        .send(
            PreparedRequestBuilder::get("http://testserver/log")
                .unwrap()
                .build(),
        )
        .unwrap();
    assert_eq!(status_code(read.get_status()), 200);

    let (_, _, body) = read.into_parts();
    assert_eq!(read_body(body).unwrap(), r#"[{"n":1},{"n":2}]"#);

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_a_body_shorter_than_its_header_when_the_client_half_closes_then_the_server_answers_400() {
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream};

    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());

    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .write_all(
            b"PUT /v1/stream/short HTTP/1.1\r\nHost: test\r\nContent-Type: text/plain\r\nContent-Length: 100\r\n\r\nabc",
        )
        .expect("write head and short body");
    stream.shutdown(Shutdown::Write).expect("half close");

    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    // The wire header declares 100 bytes but only 3 arrive, so the server
    // answers 400 and closes the connection.
    assert!(
        response.starts_with("HTTP/1.1 400"),
        "expected 400, got: {response}"
    );

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_an_oversized_body_when_the_client_declares_it_then_the_server_answers_413() {
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream};

    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());

    let mut stream = TcpStream::connect(addr).expect("connect");
    // The body is larger than the adapter cap (8 MiB). The server must refuse
    // the request and close, so the unread tail cannot corrupt the next
    // keep-alive request.
    let body_len = 9 * 1024 * 1024;
    let head = format!(
        "POST /v1/stream/oversized HTTP/1.1\r\nHost: test\r\nContent-Type: text/plain\r\nContent-Length: {body_len}\r\n\r\n"
    );
    stream.write_all(head.as_bytes()).expect("write head");
    stream.shutdown(Shutdown::Write).expect("half close");

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    let mut response = Vec::new();
    let mut buf = [0_u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                response.extend_from_slice(&buf[..n]);
                if response.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Err(error) => panic!("read response head: {error}"),
        }
    }

    let response = String::from_utf8_lossy(&response);
    assert!(
        response.starts_with("HTTP/1.1 413"),
        "expected 413 for an oversized body, got: {response}"
    );

    shutdown.turn_on();
}

/// A client can still be sending its body when the 413 comes back. The adapter
/// must read the rest of the body before it closes. A close with unread bytes
/// sends a reset (RST), and the reset can overtake the response, so the client
/// reports `fetch failed` with `write ECONNRESET` instead of the 413. This is
/// the intermittent failure of the reference server suite's large-payload case
/// (issue #1029).
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_an_oversized_body_when_the_client_keeps_writing_then_the_server_discards_it_and_closes_cleanly(
) {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let _guard = initialize_pool(42, Some(5));
    let limit = 1024;
    let (addr, shutdown) = start_server(durable_app_with_body_limit(limit));

    // Create the stream on its own connection.
    let mut create = TcpStream::connect(addr).expect("connect");
    create
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set create read timeout");
    create
        .write_all(
            b"PUT /v1/stream/linger HTTP/1.1\r\nHost: test\r\nContent-Type: application/octet-stream\r\nContent-Length: 0\r\n\r\n",
        )
        .expect("write create");
    let mut created = Vec::new();
    let mut create_buf = [0_u8; 1024];
    loop {
        match create.read(&mut create_buf) {
            Ok(0) => break,
            Ok(n) => {
                created.extend_from_slice(&create_buf[..n]);
                if created.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Err(error) => panic!("read create response: {error}"),
        }
    }
    let created = String::from_utf8_lossy(&created);
    assert!(created.starts_with("HTTP/1.1 201"), "create: {created}");

    // The body is larger than the cap and larger than the socket buffers, so
    // the client cannot finish writing before the server decides.
    let body_len = 4 * 1024 * 1024;
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .expect("set write timeout");
    stream
        .write_all(
            format!(
                "POST /v1/stream/linger HTTP/1.1\r\nHost: test\r\nContent-Type: application/octet-stream\r\nContent-Length: {body_len}\r\n\r\n"
            )
            .as_bytes(),
        )
        .expect("write head");

    // Read the 413 before sending the body, the way a client that reads the
    // error while it still uploads does.
    let (status, _) = read_raw_head(&mut stream);
    assert_eq!(status, 413, "expected 413 for an oversized body");

    // The server discards the body as it arrives, so every write succeeds.
    let chunk = vec![7_u8; 64 * 1024];
    let mut written = 0;
    while written < body_len {
        stream.write_all(&chunk).unwrap_or_else(|error| {
            panic!("the server closed before it read the body after {written} bytes: {error}")
        });
        written += chunk.len();
    }

    // With the body fully read, the close is a normal end-of-stream, not a
    // reset.
    let mut tail = [0_u8; 64];
    let read = stream.read(&mut tail).expect("read the clean close");
    assert_eq!(read, 0, "expected an end-of-stream after the body");

    shutdown.turn_on();
}

/// A chunked body has no declared length, so the adapter learns it has passed
/// the cap only after it reads part of it. It must answer 413 and discard the
/// rest. Before the fix it collected the first `limit` bytes, appended them and
/// kept the connection; the tail was then parsed as the next request.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_a_chunked_body_over_the_cap_when_the_client_keeps_writing_then_the_server_answers_413_and_discards_it(
) {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let _guard = initialize_pool(42, Some(5));
    let limit = 1024;
    let (addr, shutdown) = start_server(durable_app_with_body_limit(limit));

    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .expect("set write timeout");
    stream
        .write_all(
            b"POST /v1/stream/chunked-over-limit HTTP/1.1\r\nHost: test\r\nContent-Type: application/octet-stream\r\nTransfer-Encoding: chunked\r\n\r\n",
        )
        .expect("write head");

    // One chunk larger than the cap makes the overflow visible.
    let first = vec![7_u8; limit * 2];
    stream
        .write_all(format!("{:x}\r\n", first.len()).as_bytes())
        .expect("write first chunk size");
    stream.write_all(&first).expect("write first chunk");
    stream.write_all(b"\r\n").expect("write first chunk end");

    let (status, _) = read_raw_head(&mut stream);
    assert_eq!(status, 413, "expected 413 for a chunked body over the cap");

    // The server discards the body as it arrives, so every write succeeds.
    let chunk = vec![7_u8; 64 * 1024];
    let mut remaining = 2 * 1024 * 1024;
    while remaining > 0 {
        stream
            .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
            .expect("write chunk size");
        stream.write_all(&chunk).unwrap_or_else(|error| {
            panic!("the server closed before it read the chunked body: {error}")
        });
        stream.write_all(b"\r\n").expect("write chunk end");
        remaining -= chunk.len();
    }
    stream.write_all(b"0\r\n\r\n").expect("write last chunk");

    let mut tail = [0_u8; 64];
    let read = stream.read(&mut tail).expect("read the clean close");
    assert_eq!(read, 0, "expected an end-of-stream after the body");

    shutdown.turn_on();
}

/// A chunked body can end before its last chunk, for example when the client
/// disconnects. A partial append would store corrupt data, so the adapter
/// answers 400 and stores nothing.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_a_chunked_body_that_ends_early_when_the_client_half_closes_then_the_server_answers_400_and_stores_nothing(
) {
    use std::io::Write;
    use std::net::{Shutdown, TcpStream};

    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());

    let mut create = TcpStream::connect(addr).expect("connect");
    create
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set create read timeout");
    create
        .write_all(
            b"PUT /v1/stream/early HTTP/1.1\r\nHost: test\r\nContent-Type: text/plain\r\nContent-Length: 0\r\n\r\n",
        )
        .expect("write create");
    let (status, _) = read_raw_head(&mut create);
    assert!(status == 200 || status == 201, "create status {status}");

    // One chunk, but no last chunk: the body ends with the half-close.
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    stream
        .write_all(
            b"POST /v1/stream/early HTTP/1.1\r\nHost: test\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\n\r\n7\r\npartial\r\n",
        )
        .expect("write partial chunked body");
    stream.shutdown(Shutdown::Write).expect("half close");
    let (status, _) = read_raw_head(&mut stream);
    assert_eq!(status, 400, "an early end must be refused");

    // The stream stays empty: no partial append.
    let mut get = TcpStream::connect(addr).expect("connect");
    get.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    get.write_all(b"GET /v1/stream/early HTTP/1.1\r\nHost: test\r\n\r\n")
        .expect("write get");
    let (status, content_length) = read_raw_head(&mut get);
    assert_eq!(status, 200, "get status");
    assert_eq!(content_length, 0, "a partial append must not be stored");

    shutdown.turn_on();
}

/// A client can stop sending after it reads the 413. The discard must not hold
/// the connection past `DurableHttpConfig::linger_timeout`.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_a_rejected_body_when_the_client_stops_sending_then_the_server_closes_at_the_linger_timeout(
) {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let _guard = initialize_pool(42, Some(5));
    let limit = 1024;
    let config = DurableHttpConfig::default().with_linger_timeout(Duration::from_millis(200));
    let (addr, shutdown) = start_server(durable_app_with_http_config(
        DurableLimits::default().with_max_body_bytes(limit),
        config,
    ));

    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    let body_len = 1024 * 1024;
    stream
        .write_all(
            format!(
                "POST /v1/stream/linger-timeout HTTP/1.1\r\nHost: test\r\nContent-Type: application/octet-stream\r\nContent-Length: {body_len}\r\n\r\n"
            )
            .as_bytes(),
        )
        .expect("write head");
    stream
        .write_all(&vec![7_u8; 4096])
        .expect("write body part");

    let (status, _) = read_raw_head(&mut stream);
    assert_eq!(status, 413, "expected 413 for an oversized body");

    // Stop sending. The discard times out and the server closes.
    let started = Ctx::host().clock().now();
    let mut tail = [0_u8; 64];
    let read = stream.read(&mut tail).expect("read the close");
    assert_eq!(read, 0, "expected an end-of-stream after the timeout");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the server held the connection for {:?}",
        started.elapsed()
    );

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn oversized_body_answers_413_from_the_limit() {
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream};

    let _guard = initialize_pool(42, Some(5));
    // A small configured cap proves the adapter reads the limit from the
    // service, not a built-in constant.
    let limit = 64_usize;
    let (addr, shutdown) = start_server(durable_app_with_body_limit(limit));

    let mut stream = TcpStream::connect(addr).expect("connect");
    let body_len = limit + 1;
    let head = format!(
        "POST /v1/stream/over-limit HTTP/1.1\r\nHost: test\r\nContent-Type: application/json\r\nContent-Length: {body_len}\r\n\r\n"
    );
    stream.write_all(head.as_bytes()).expect("write head");
    stream.shutdown(Shutdown::Write).expect("half close");

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    let mut response = Vec::new();
    let mut buf = [0_u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                response.extend_from_slice(&buf[..n]);
                if response.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Err(error) => panic!("read response head: {error}"),
        }
    }

    let response = String::from_utf8_lossy(&response);
    assert!(
        response.starts_with("HTTP/1.1 413"),
        "expected 413 for a body over the configured limit, got: {response}"
    );

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn body_at_the_limit_passes() {
    let _guard = initialize_pool(42, Some(5));
    let limit = 64_usize;
    let (addr, shutdown) = start_server(durable_app_with_body_limit(limit));
    let client = make_client(addr);

    // The empty JSON object form `{"data":""}` is 11 bytes, so the filler makes
    // a valid JSON body of exactly the configured limit.
    let body = format!("{{\"data\":\"{}\"}}", "x".repeat(limit - 11));
    assert_eq!(body.len(), limit, "the test body fills the limit exactly");

    let created = client
        .send(
            PreparedRequestBuilder::put("http://testserver/limit")
                .unwrap()
                .header(SimpleHeader::CONTENT_TYPE, "application/json")
                .build(),
        )
        .unwrap();
    assert_eq!(status_code(created.get_status()), 201);

    let appended = client
        .send(
            PreparedRequestBuilder::post("http://testserver/limit")
                .unwrap()
                .header(SimpleHeader::CONTENT_TYPE, "application/json")
                .body_text(body)
                .build(),
        )
        .unwrap();
    assert_eq!(
        status_code(appended.get_status()),
        204,
        "a body at the configured limit must pass"
    );

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn given_many_put_get_pairs_on_one_connection_when_they_run_then_every_read_is_200() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;

    /// Read one response and name the request in any failure, so an
    /// intermittent reset says which step of which pair broke the connection.
    fn read_response(reader: &mut BufReader<TcpStream>, request: &str) -> (u16, String) {
        let mut status_line = String::new();
        reader
            .read_line(&mut status_line)
            .unwrap_or_else(|error| panic!("{request} response status line: {error}"));
        let status: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let mut content_length = 0_usize;
        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .unwrap_or_else(|error| panic!("{request} response header: {error}"));
            if line == "\r\n" || line.is_empty() {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0_u8; content_length];
        reader
            .read_exact(&mut body)
            .unwrap_or_else(|error| panic!("{request} response body: {error}"));
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());

    let stream = TcpStream::connect(addr).expect("connect");
    let mut writer = stream.try_clone().expect("clone");
    let mut reader = BufReader::new(stream);

    for iteration in 0..20 {
        writer
            .write_all(
                b"PUT /v1/stream/keepalive HTTP/1.1\r\nHost: test\r\nContent-Type: text/plain\r\nContent-Length: 7\r\n\r\ninitial",
            )
            .unwrap_or_else(|error| panic!("pair {iteration} put write: {error}"));
        let (status, _) = read_response(&mut reader, &format!("pair {iteration} put"));
        assert!(status == 200 || status == 201, "put status {status}");

        writer
            .write_all(b"GET /v1/stream/keepalive HTTP/1.1\r\nHost: test\r\n\r\n")
            .unwrap_or_else(|error| panic!("pair {iteration} get write: {error}"));
        let (status, body) = read_response(&mut reader, &format!("pair {iteration} get"));
        assert_eq!(status, 200, "get status");
        assert_eq!(body, "initial");
    }

    shutdown.turn_on();
}

/// A keep-alive read re-arms the socket edge before each read attempt. After a
/// large frame is drained, a smaller frame must arm the readiness again: the
/// kqueue selector reports a readable event only when the byte count grows
/// unless the edge was re-armed. Re-arming after the frame lands (at the park)
/// instead leaves the reader parked until its deadline — the keep-alive stall
/// of issue #1127.
#[test]
#[serial(http_test)]
fn given_a_drained_large_frame_when_a_smaller_one_arrives_then_the_socket_readiness_arms_again() {
    use std::io::{Read, Write};
    use std::os::unix::io::AsRawFd;
    use std::os::unix::net::UnixStream;

    use foundation_core::valtron::EventReadiness;
    use foundation_iogate::FdReadiness;

    /// Wait (bounded) for the reactor's drain thread to latch readiness.
    fn wait_ready(readiness: &FdReadiness, ctx: &Ctx, what: &str) {
        let deadline = ctx.clock().now() + Duration::from_secs(2);
        while !readiness.is_ready(None) {
            assert!(
                ctx.clock().now() < deadline,
                "{what}: the socket readiness never armed"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    let ctx = Ctx::host();
    let (mut reader, mut writer) = UnixStream::pair().expect("socketpair");
    reader.set_nonblocking(true).expect("set nonblocking");
    let fd = reader.as_raw_fd();

    let readiness = FdReadiness::register(fd).expect("register the socket");
    writer
        .write_all(&[7_u8; 100])
        .expect("write the large frame");
    wait_ready(&readiness, &ctx, "large frame");

    let mut buf = [0_u8; 256];
    assert_eq!(
        reader.read(&mut buf).expect("read the large frame"),
        100,
        "the large frame drains"
    );

    // The idle read re-arms before it reads, so a smaller frame arms again.
    readiness.clear();
    writer
        .write_all(&[7_u8; 50])
        .expect("write the small frame");
    wait_ready(&readiness, &ctx, "small frame after the drained large one");
    assert_eq!(
        reader.read(&mut buf).expect("read the small frame"),
        50,
        "the small frame is readable"
    );
}
