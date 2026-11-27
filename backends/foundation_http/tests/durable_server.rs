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
use foundation_http::native::durable::DurableHandler;
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

fn start_server(app: HttpApp<Arc<dyn Serve>>) -> (std::net::SocketAddr, Arc<ShutdownSignal>) {
    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");

    let config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
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

fn header(response: &SimpleResponse<SendSafeBody>, name: SimpleHeader) -> Option<String> {
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
        header(&created, SimpleHeader::LOCATION).as_deref(),
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
    let next = header(&appended, SimpleHeader::custom("stream-next-offset"))
        .expect("next offset");

    let read = client
        .send(
            PreparedRequestBuilder::get("http://testserver/events")
                .unwrap()
                .build(),
        )
        .unwrap();
    assert_eq!(status_code(read.get_status()), 200);
    assert_eq!(
        header(&read, SimpleHeader::custom("stream-up-to-date")).as_deref(),
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
        header(&head, SimpleHeader::custom("stream-next-offset")).as_deref(),
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
#[foundation_macros::timeout(120000)]
#[serial(http_test)]
fn given_many_put_get_pairs_on_one_connection_when_they_run_then_every_read_is_200() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;

    fn read_response(reader: &mut BufReader<TcpStream>) -> (u16, String) {
        let mut status_line = String::new();
        reader.read_line(&mut status_line).expect("status line");
        let status: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let mut content_length = 0_usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("header");
            if line == "\r\n" || line.is_empty() {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0_u8; content_length];
        reader.read_exact(&mut body).expect("body");
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());

    let stream = TcpStream::connect(addr).expect("connect");
    let mut writer = stream.try_clone().expect("clone");
    let mut reader = BufReader::new(stream);

    for _ in 0..20 {
        writer
            .write_all(
                b"PUT /v1/stream/keepalive HTTP/1.1\r\nHost: test\r\nContent-Type: text/plain\r\nContent-Length: 7\r\n\r\ninitial",
            )
            .expect("put");
        let (status, _) = read_response(&mut reader);
        assert!(status == 200 || status == 201, "put status {status}");

        writer
            .write_all(b"GET /v1/stream/keepalive HTTP/1.1\r\nHost: test\r\n\r\n")
            .expect("get");
        let (status, body) = read_response(&mut reader);
        assert_eq!(status, 200, "get status");
        assert_eq!(body, "initial");
    }

    shutdown.turn_on();
}
