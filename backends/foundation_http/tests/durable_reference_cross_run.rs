//! Cross-implementation run: the reference Rust client against our server.
//!
//! Each test starts a real `HttpServer` with the catch-all Durable handler over
//! the memory VFS, then drives it with the reference client from
//! `packages/client-rust`. The dependency is the public repository, pinned by
//! commit, because the client is not published on crates.io.
//!
//! The scenarios mirror `durable_cross_client.rs`: create, append, catch-up
//! read, long-poll, Server-Sent Events (SSE), close and fork.

#![cfg(all(feature = "multi", unix))]

use std::sync::Arc;
use std::time::Duration;

use durable_streams::{Client, CloseOptions, CreateOptions, LiveMode, Offset};
use foundation_core::durable::{DurableStream, MemoryVfs};
use foundation_core::valtron::initialize_pool;
use foundation_http::native::durable::DurableHandler;
use foundation_http::native::serve::Serve;
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::ShutdownSignal;
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

/// The reference client needs a runtime with I/O and timers.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime")
}

fn reference_client(addr: std::net::SocketAddr) -> Client {
    Client::builder()
        .base_url(format!("http://{addr}"))
        .build()
        .expect("reference client")
}

/// Read from the start and stop at the tail.
async fn read_all(stream: &durable_streams::DurableStream) -> Vec<u8> {
    let mut reader = stream
        .read()
        .offset(Offset::Beginning)
        .build()
        .expect("reader");
    let mut out = Vec::new();
    while let Some(chunk) = reader.next_chunk().await.expect("a chunk") {
        out.extend_from_slice(&chunk.data);
        if chunk.up_to_date {
            break;
        }
    }
    out
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn given_the_reference_client_when_it_creates_appends_and_reads_then_bytes_are_exact() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = reference_client(addr);

    runtime().block_on(async {
        let stream = client.stream("/ref/roundtrip");
        stream.create().await.expect("create");
        stream.append("hello ").await.expect("first append");
        let last = stream.append("world").await.expect("second append");
        assert_ne!(
            last.next_offset.as_str(),
            "-1",
            "append reports a real next offset"
        );

        let bytes = read_all(&stream).await;
        assert_eq!(bytes, b"hello world", "byte-exact read, got {bytes:?}");
    });

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn given_the_reference_client_at_the_tail_when_an_append_arrives_then_long_poll_returns_it() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = reference_client(addr);

    runtime().block_on(async {
        let stream = client.stream("/ref/long-poll");
        stream.create().await.expect("create");
        stream.append("first").await.expect("append");

        let mut catch_up = stream
            .read()
            .offset(Offset::Beginning)
            .build()
            .expect("reader");
        while let Some(chunk) = catch_up.next_chunk().await.expect("a chunk") {
            if chunk.up_to_date {
                break;
            }
        }
        let tail = catch_up.offset().clone();

        let mut poll = stream
            .read()
            .offset(tail)
            .live(LiveMode::LongPoll)
            .timeout(Duration::from_secs(20))
            .build()
            .expect("long-poll reader");
        let writer = stream.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            writer.append("late").await.expect("late append");
        });

        let chunk = poll
            .next_chunk()
            .await
            .expect("long-poll step")
            .expect("the new data");
        assert_eq!(
            &chunk.data[..],
            b"late",
            "the long poll must return the new data"
        );
    });

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn given_a_binary_stream_when_the_reference_client_reads_sse_then_base64_data_decodes() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = reference_client(addr);

    runtime().block_on(async {
        let stream = client.stream("/ref/sse");
        stream
            .create_with(CreateOptions::new().content_type("application/octet-stream"))
            .await
            .expect("create");
        stream
            .append(vec![0x00_u8, 0xff, 0x10])
            .await
            .expect("append");
        stream.close().await.expect("close");

        let mut sse = stream
            .read()
            .offset(Offset::Beginning)
            .live(LiveMode::Sse)
            .build()
            .expect("SSE reader");
        let chunk = sse
            .next_chunk()
            .await
            .expect("SSE step")
            .expect("one data event");
        // The raw base64 text would not equal these bytes, so equality proves
        // that the client honoured the `stream-sse-data-encoding` header.
        assert_eq!(
            &chunk.data[..],
            &[0x00_u8, 0xff, 0x10],
            "the decoded SSE bytes must match"
        );
    });

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn given_the_reference_client_closes_a_stream_then_a_read_reports_the_closure() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = reference_client(addr);

    runtime().block_on(async {
        let stream = client.stream("/ref/closed");
        stream.create().await.expect("create");
        stream.append("a").await.expect("append");
        stream
            .close_with(CloseOptions::new().data("done"))
            .await
            .expect("close");

        let bytes = read_all(&stream).await;
        assert_eq!(
            bytes, b"adone",
            "the final message is part of the log, got {bytes:?}"
        );
        let head = stream.head().await.expect("head");
        assert!(head.stream_closed, "the head must report the closure");
    });

    shutdown.turn_on();
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn given_a_source_stream_when_the_reference_client_creates_a_fork_then_it_reads_the_prefix() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());
    let client = reference_client(addr);

    runtime().block_on(async {
        let source = client.stream("/ref/fork-source");
        source.create().await.expect("create source");
        source.append("0123").await.expect("append source");

        let mut reader = source
            .read()
            .offset(Offset::Beginning)
            .build()
            .expect("source reader");
        while let Some(chunk) = reader.next_chunk().await.expect("a chunk") {
            if chunk.up_to_date {
                break;
            }
        }
        let fork_at = reader.offset().as_str().to_string();

        let child = client.stream("/ref/fork-child");
        child
            .create_with(
                CreateOptions::new()
                    .content_type("application/octet-stream")
                    .header("Stream-Forked-From", "/ref/fork-source")
                    .header("Stream-Fork-Offset", &fork_at),
            )
            .await
            .expect("create fork");

        let prefix = read_all(&child).await;
        assert_eq!(
            prefix, b"0123",
            "the fork inherits the prefix, got {prefix:?}"
        );

        child.append("45").await.expect("append fork");
        let grown = read_all(&child).await;
        assert_eq!(
            grown, b"012345",
            "the fork grows past its prefix, got {grown:?}"
        );

        let unchanged = read_all(&source).await;
        assert_eq!(
            unchanged, b"0123",
            "the source does not see the fork, got {unchanged:?}"
        );
    });

    shutdown.turn_on();
}
