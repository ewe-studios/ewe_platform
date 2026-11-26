//! End-to-end tests for the Durable Streams HTTP surface.
//!
//! Each test starts a real `HttpServer` with the catch-all Durable handler over
//! the memory VFS, then drives it with the platform client.

#![cfg(all(feature = "multi", unix))]

use std::sync::Arc;
use std::time::Duration;

use foundation_core::durable::{DurableStream, MemoryVfs};
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
    let _ = ctx.get_or_insert(DurableStream::new(Arc::new(MemoryVfs::new())));
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
    assert_eq!(
        header(&created, SimpleHeader::LOCATION).as_deref(),
        Some("/events")
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
fn given_a_body_shorter_than_its_header_when_the_client_half_closes_then_the_server_uses_the_bytes_it_read() {
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
    // The server rewrites the request framing to chunked and reads to the
    // end of the half-closed connection, so the 3 bytes are the whole body.
    assert!(
        response.starts_with("HTTP/1.1 201"),
        "expected 201, got: {response}"
    );

    shutdown.turn_on();
}
