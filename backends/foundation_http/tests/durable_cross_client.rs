//! Cross-implementation run: our Durable Streams client against our server.
//!
//! Each test starts a real `HttpServer` with the catch-all Durable handler over
//! the memory VFS, then drives it with the client core over [`NetioTransport`].
//! Every scenario asserts the result, not only the status.

#![cfg(all(feature = "multi", unix))]

use std::sync::Arc;
use std::time::Duration;

use foundation_core::durable::client::{ClientConfig, ClientError, DurableClient, ReadMode};
use foundation_core::durable::{DurableStream, MemoryVfs, StreamName};
use foundation_core::valtron::{block_on_future_scoped, initialize_pool};
use foundation_http::native::durable::DurableHandler;
use foundation_http::native::serve::Serve;
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::ShutdownSignal;
use foundation_netio::native::durable::NetioTransport;
use foundation_netio::{DynNetClient, HttpClientBuilder};
use serial_test::serial;

/// Start one in-process server on an ephemeral loopback port.
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

fn durable_app() -> HttpApp<Arc<dyn Serve>> {
    let ctx = Ctx::host();
    let _ = ctx.get_or_insert(DurableStream::new(Arc::new(MemoryVfs::new()), &ctx));
    let mut app = HttpApp::with_context(ctx);
    app.fallback::<DurableHandler>();
    app
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

fn durable_client(addr: std::net::SocketAddr) -> DurableClient {
    DurableClient::new(
        Arc::new(NetioTransport::from_client(make_client(addr))),
        ClientConfig {
            base_url: "http://testserver".to_string(),
            default_headers: Vec::new(),
        },
    )
}

fn stream_name(raw: &str) -> StreamName {
    StreamName::parse(raw).expect("a valid stream name")
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_our_client_and_server_when_it_creates_appends_and_reads_then_bytes_are_exact() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = durable_client(addr);
    let handle = client.stream(&stream_name("cross/roundtrip"));

    block_on_future_scoped(async {
        let created = handle
            .create(
                Some("application/octet-stream"),
                None,
                None,
                false,
                Vec::new(),
            )
            .await
            .expect("create");
        assert_eq!(created.status, 201, "create status");
        assert!(
            created.offset.is_some(),
            "create reports the tail offset, got {:?}",
            created.offset
        );

        let first = handle
            .append(
                b"hello ".to_vec(),
                Some("application/octet-stream"),
                None,
                None,
                false,
            )
            .await
            .expect("first append");
        assert_eq!(first.status, 204, "append status");
        let middle = handle
            .append(
                b"world".to_vec(),
                Some("application/octet-stream"),
                None,
                None,
                false,
            )
            .await
            .expect("second append");
        assert!(
            middle.offset.is_some(),
            "append reports the next offset, got {:?}",
            middle.offset
        );

        let read = handle.read().send().await.expect("catch-up read");
        assert_eq!(read.status, 200, "read status");
        assert!(read.up_to_date, "a catch-up read reaches the tail");
        let bytes: Vec<u8> = read
            .chunks
            .iter()
            .flat_map(|chunk| chunk.data.iter().copied())
            .collect();
        assert_eq!(
            bytes, b"hello world",
            "the read must be byte-exact, got {bytes:?}"
        );
        assert!(
            read.next_offset.is_some(),
            "the read reports the next offset"
        );
    });

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_our_client_at_the_tail_when_an_append_arrives_then_long_poll_returns_it() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = durable_client(addr);
    let handle = client.stream(&stream_name("cross/long-poll"));

    block_on_future_scoped(async {
        handle
            .create(
                Some("application/octet-stream"),
                None,
                None,
                false,
                Vec::new(),
            )
            .await
            .expect("create");
        handle
            .append(
                b"first".to_vec(),
                Some("application/octet-stream"),
                None,
                None,
                false,
            )
            .await
            .expect("append");

        let catch_up = handle.read().send().await.expect("catch-up read");
        let tail = catch_up.next_offset.expect("the tail offset");

        let writer = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            block_on_future_scoped(async move {
                writer
                    .append(
                        b"late".to_vec(),
                        Some("application/octet-stream"),
                        None,
                        None,
                        false,
                    )
                    .await
                    .expect("late append");
            });
        });

        let polled = handle
            .read()
            .offset(tail)
            .mode(ReadMode::LongPoll)
            .send()
            .await
            .expect("long-poll read");
        assert_eq!(polled.status, 200, "long-poll status");
        let bytes: Vec<u8> = polled
            .chunks
            .iter()
            .flat_map(|chunk| chunk.data.iter().copied())
            .collect();
        assert_eq!(
            bytes, b"late",
            "the long poll must return the new data, got {bytes:?}"
        );
    });

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_a_binary_stream_when_our_client_reads_sse_then_the_encoding_header_is_base64() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = durable_client(addr);
    let handle = client.stream(&stream_name("cross/sse"));

    block_on_future_scoped(async {
        handle
            .create(
                Some("application/octet-stream"),
                None,
                None,
                false,
                Vec::new(),
            )
            .await
            .expect("create");
        // Bytes that are not valid UTF-8 prove that the Server-Sent Events (SSE)
        // payload went over the wire as base64.
        handle
            .append(
                vec![0x00, 0xff, 0x10],
                Some("application/octet-stream"),
                None,
                None,
                false,
            )
            .await
            .expect("append");
        handle.close(Vec::new(), None).await.expect("close");

        let mut sse = handle
            .read()
            .mode(ReadMode::Sse)
            .open_sse()
            .await
            .expect("open SSE");
        assert_eq!(sse.status(), 200, "SSE status");
        assert_eq!(
            sse.header("stream-sse-data-encoding"),
            Some("base64"),
            "a binary stream must announce base64 data events"
        );

        let chunk = sse.next().await.expect("SSE step").expect("one data event");
        assert_eq!(
            chunk.data,
            vec![0x00, 0xff, 0x10],
            "the decoded bytes must match"
        );
        assert!(sse.closed(), "the control event reports the closure");
        assert!(
            sse.next().await.expect("SSE end").is_none(),
            "the closed stream ends"
        );
    });

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_our_client_closes_a_stream_when_it_reads_then_the_read_reports_the_closure() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = durable_client(addr);
    let handle = client.stream(&stream_name("cross/closed"));

    block_on_future_scoped(async {
        handle
            .create(Some("text/plain"), None, None, false, Vec::new())
            .await
            .expect("create");
        handle
            .append(b"a".to_vec(), Some("text/plain"), None, None, false)
            .await
            .expect("append");
        let closed = handle
            .close(b"done".to_vec(), Some("text/plain"))
            .await
            .expect("close");
        assert_eq!(closed.status, 204, "close status");

        let read = handle.read().send().await.expect("read after close");
        assert!(read.closed, "the read must report the closure");
        assert!(read.up_to_date, "a closed read is at the tail");
        let bytes: Vec<u8> = read
            .chunks
            .iter()
            .flat_map(|chunk| chunk.data.iter().copied())
            .collect();
        assert_eq!(bytes, b"adone", "the final message is part of the log");

        let refused = handle
            .append(b"more".to_vec(), Some("text/plain"), None, None, false)
            .await;
        assert!(
            matches!(refused, Err(ClientError::Closed)),
            "an append after a close must report Closed, got {refused:?}"
        );
    });

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn given_a_source_stream_when_our_client_creates_a_fork_then_it_reads_the_prefix_and_grows() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = durable_client(addr);
    let source = client.stream(&stream_name("cross/fork-source"));
    let child = client.stream(&stream_name("cross/fork-child"));

    block_on_future_scoped(async {
        source
            .create(
                Some("application/octet-stream"),
                None,
                None,
                false,
                Vec::new(),
            )
            .await
            .expect("create source");
        source
            .append(
                b"0123".to_vec(),
                Some("application/octet-stream"),
                None,
                None,
                false,
            )
            .await
            .expect("append source");
        let tail = source
            .read()
            .send()
            .await
            .expect("read source")
            .next_offset
            .expect("the source tail offset");

        child
            .with_headers(vec![
                (
                    "stream-forked-from".to_string(),
                    "cross/fork-source".to_string(),
                ),
                ("stream-fork-offset".to_string(), tail),
            ])
            .create(
                Some("application/octet-stream"),
                None,
                None,
                false,
                Vec::new(),
            )
            .await
            .expect("create fork");

        let prefix = child.read().send().await.expect("read fork");
        let bytes: Vec<u8> = prefix
            .chunks
            .iter()
            .flat_map(|chunk| chunk.data.iter().copied())
            .collect();
        assert_eq!(
            bytes, b"0123",
            "the fork inherits the source prefix, got {bytes:?}"
        );

        child
            .append(
                b"45".to_vec(),
                Some("application/octet-stream"),
                None,
                None,
                false,
            )
            .await
            .expect("append fork");
        let grown = child.read().send().await.expect("read grown fork");
        let bytes: Vec<u8> = grown
            .chunks
            .iter()
            .flat_map(|chunk| chunk.data.iter().copied())
            .collect();
        assert_eq!(
            bytes, b"012345",
            "the fork grows past its prefix, got {bytes:?}"
        );

        let unchanged = source.read().send().await.expect("read source again");
        let bytes: Vec<u8> = unchanged
            .chunks
            .iter()
            .flat_map(|chunk| chunk.data.iter().copied())
            .collect();
        assert_eq!(
            bytes, b"0123",
            "the source does not see the fork append, got {bytes:?}"
        );
    });

    shutdown.turn_on();
}
