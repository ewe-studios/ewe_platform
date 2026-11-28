//! Differential oracle: our Durable Streams client and the reference client.
//!
//! One in-process server runs the catch-all Durable handler over the memory
//! VFS. Both clients run the same scenario on their own stream paths. The test
//! compares the observed outcomes, which are the exact bytes, the presence and
//! the order of the next offsets, and the read results. It does not compare the
//! opaque offset text between the two implementations.

#![cfg(all(feature = "multi", unix))]

use std::sync::Arc;
use std::time::Duration;

use durable_streams::{Client, CloseOptions, CreateOptions, Offset};
use foundation_core::durable::client::{ClientConfig, DurableClient, ReadChunk, ReadResult};
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

/// One read and what it proves about the stream.
#[derive(Debug, PartialEq, Eq)]
struct ReadOutcome {
    bytes: Vec<u8>,
    up_to_date: bool,
    next_offset_present: bool,
}

/// The result of create.
#[derive(Debug, PartialEq, Eq)]
struct CreateObservation {
    created: bool,
    head_offset_present: bool,
}

/// The result of the two appends.
#[derive(Debug, PartialEq, Eq)]
struct AppendObservation {
    offset_present: Vec<bool>,
    strictly_increasing: bool,
}

/// The result of close and the read after it.
#[derive(Debug, PartialEq, Eq)]
struct CloseObservation {
    offset_present: bool,
    offset_after_last_append: bool,
    read: ReadOutcome,
    head_closed: bool,
}

/// The result of the fork steps.
#[derive(Debug, PartialEq, Eq)]
struct ForkObservation {
    prefix: ReadOutcome,
    grown: ReadOutcome,
    source_after: ReadOutcome,
}

/// The main stream steps, without the fork.
#[derive(Debug, PartialEq, Eq)]
struct MainObservation {
    create: CreateObservation,
    append: AppendObservation,
    catch_up: ReadOutcome,
    from_offset: ReadOutcome,
    close: CloseObservation,
}

/// What one implementation observes in the scenario.
///
/// The offset text stays out of the structure. An offset appears only as a
/// presence flag or as an order result, so the two implementations compare on
/// meaning and not on spelling.
#[derive(Debug, PartialEq, Eq)]
struct Trace {
    main: MainObservation,
    fork: ForkObservation,
}

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

fn concat(chunks: &[ReadChunk]) -> Vec<u8> {
    chunks
        .iter()
        .flat_map(|chunk| chunk.data.iter().copied())
        .collect()
}

fn our_read(result: &ReadResult) -> ReadOutcome {
    ReadOutcome {
        bytes: concat(&result.chunks),
        up_to_date: result.up_to_date,
        next_offset_present: result.next_offset.is_some(),
    }
}

/// Read the reference stream to its tail.
async fn reference_read(stream: &durable_streams::DurableStream, from: Offset) -> ReadOutcome {
    let mut reader = stream
        .read()
        .offset(from)
        .build()
        .expect("reference reader");
    let mut outcome = ReadOutcome {
        bytes: Vec::new(),
        up_to_date: false,
        next_offset_present: false,
    };
    while let Some(chunk) = reader.next_chunk().await.expect("a reference chunk") {
        outcome.bytes.extend_from_slice(&chunk.data);
        outcome.next_offset_present |= !matches!(chunk.next_offset, Offset::Beginning);
        if chunk.up_to_date {
            outcome.up_to_date = true;
            break;
        }
    }
    outcome
}

/// Create, append, read, read from an offset and close on our client.
async fn our_main(client: &DurableClient) -> MainObservation {
    let stream = client.stream(&stream_name("diff/ours/stream"));
    stream
        .create(
            Some("application/octet-stream"),
            None,
            None,
            false,
            Vec::new(),
        )
        .await
        .expect("create");
    let create = CreateObservation {
        created: true,
        head_offset_present: stream
            .head()
            .await
            .expect("head after create")
            .offset
            .is_some(),
    };

    let first = stream
        .append(
            b"hello ".to_vec(),
            Some("application/octet-stream"),
            None,
            None,
            false,
        )
        .await
        .expect("first append");
    let second = stream
        .append(
            b"world".to_vec(),
            Some("application/octet-stream"),
            None,
            None,
            false,
        )
        .await
        .expect("second append");
    let append = AppendObservation {
        offset_present: vec![first.offset.is_some(), second.offset.is_some()],
        strictly_increasing: match (&first.offset, &second.offset) {
            (Some(first), Some(second)) => first < second,
            _ => false,
        },
    };
    let first_offset = first.offset.expect("the first append offset");

    let catch_up = our_read(&stream.read().send().await.expect("catch-up read"));
    let from_offset = our_read(
        &stream
            .read()
            .offset(first_offset)
            .send()
            .await
            .expect("read from an offset"),
    );

    let closed = stream
        .close(b"done".to_vec(), Some("application/octet-stream"))
        .await
        .expect("close");
    let close_offset_after_last_append = match (&second.offset, &closed.offset) {
        (Some(last_append), Some(close)) => last_append < close,
        _ => false,
    };
    let close = CloseObservation {
        offset_present: closed.offset.is_some(),
        offset_after_last_append: close_offset_after_last_append,
        read: our_read(&stream.read().send().await.expect("read after close")),
        head_closed: stream.head().await.expect("head after close").closed,
    };

    MainObservation {
        create,
        append,
        catch_up,
        from_offset,
        close,
    }
}

/// Fork a source stream on our client.
async fn our_fork(client: &DurableClient) -> ForkObservation {
    let source = client.stream(&stream_name("diff/ours/fork-source"));
    source
        .create(
            Some("application/octet-stream"),
            None,
            None,
            false,
            Vec::new(),
        )
        .await
        .expect("create fork source");
    source
        .append(
            b"0123".to_vec(),
            Some("application/octet-stream"),
            None,
            None,
            false,
        )
        .await
        .expect("append fork source");
    let fork_at = source
        .read()
        .send()
        .await
        .expect("read fork source")
        .next_offset
        .expect("the fork offset");

    let child = client
        .stream(&stream_name("diff/ours/fork-child"))
        .with_headers(vec![
            (
                "stream-forked-from".to_string(),
                "diff/ours/fork-source".to_string(),
            ),
            ("stream-fork-offset".to_string(), fork_at),
        ]);
    child
        .create(
            Some("application/octet-stream"),
            None,
            None,
            false,
            Vec::new(),
        )
        .await
        .expect("create fork child");
    let prefix = our_read(&child.read().send().await.expect("read the fork prefix"));

    child
        .append(
            b"45".to_vec(),
            Some("application/octet-stream"),
            None,
            None,
            false,
        )
        .await
        .expect("append to the fork");
    let grown = our_read(&child.read().send().await.expect("read the grown fork"));
    let source_after = our_read(
        &source
            .read()
            .send()
            .await
            .expect("read the source after the fork"),
    );

    ForkObservation {
        prefix,
        grown,
        source_after,
    }
}

async fn our_trace(addr: std::net::SocketAddr) -> Trace {
    let client = durable_client(addr);
    Trace {
        main: our_main(&client).await,
        fork: our_fork(&client).await,
    }
}

fn reference_client(addr: std::net::SocketAddr) -> Client {
    Client::builder()
        .base_url(format!("http://{addr}"))
        .build()
        .expect("reference client")
}

/// Read the tail offset of the reference source stream.
async fn reference_tail(stream: &durable_streams::DurableStream) -> String {
    let mut reader = stream
        .read()
        .offset(Offset::Beginning)
        .build()
        .expect("reference source reader");
    let mut tail = String::new();
    while let Some(chunk) = reader.next_chunk().await.expect("a source chunk") {
        tail = chunk.next_offset.as_str().to_string();
        if chunk.up_to_date {
            break;
        }
    }
    tail
}

/// Create, append, read, read from an offset and close on the reference client.
async fn reference_main(client: &Client) -> MainObservation {
    let stream = client.stream("/diff/reference/stream");
    stream
        .create_with(CreateOptions::new().content_type("application/octet-stream"))
        .await
        .expect("reference create");
    let create = CreateObservation {
        created: true,
        head_offset_present: !matches!(
            stream.head().await.expect("reference head").next_offset,
            Offset::Beginning
        ),
    };

    let first = stream
        .append("hello ")
        .await
        .expect("reference first append");
    let second = stream
        .append("world")
        .await
        .expect("reference second append");
    let first_offset = first.next_offset.as_str().to_string();
    let second_offset = second.next_offset.as_str().to_string();
    let append = AppendObservation {
        offset_present: vec![
            !matches!(first.next_offset, Offset::Beginning),
            !matches!(second.next_offset, Offset::Beginning),
        ],
        strictly_increasing: first_offset < second_offset,
    };

    let catch_up = reference_read(&stream, Offset::Beginning).await;
    let from_offset = reference_read(&stream, Offset::at(first_offset)).await;

    let closed = stream
        .close_with(CloseOptions::new().data("done"))
        .await
        .expect("reference close");
    let close_offset_after_last_append = second_offset.as_str() < closed.final_offset.as_str();
    let close = CloseObservation {
        offset_present: !matches!(closed.final_offset, Offset::Beginning),
        offset_after_last_append: close_offset_after_last_append,
        read: reference_read(&stream, Offset::Beginning).await,
        head_closed: stream
            .head()
            .await
            .expect("reference head after close")
            .stream_closed,
    };

    MainObservation {
        create,
        append,
        catch_up,
        from_offset,
        close,
    }
}

/// Fork a source stream on the reference client.
async fn reference_fork(client: &Client) -> ForkObservation {
    let source = client.stream("/diff/reference/fork-source");
    source
        .create_with(CreateOptions::new().content_type("application/octet-stream"))
        .await
        .expect("reference create fork source");
    source
        .append("0123")
        .await
        .expect("reference append source");
    let fork_at = reference_tail(&source).await;

    let child = client.stream("/diff/reference/fork-child");
    child
        .create_with(
            CreateOptions::new()
                .content_type("application/octet-stream")
                .header("Stream-Forked-From", "/diff/reference/fork-source")
                .header("Stream-Fork-Offset", &fork_at),
        )
        .await
        .expect("reference create fork child");
    let prefix = reference_read(&child, Offset::Beginning).await;

    child
        .append("45")
        .await
        .expect("reference append to the fork");
    let grown = reference_read(&child, Offset::Beginning).await;
    let source_after = reference_read(&source, Offset::Beginning).await;

    ForkObservation {
        prefix,
        grown,
        source_after,
    }
}

async fn reference_trace(addr: std::net::SocketAddr) -> Trace {
    let client = reference_client(addr);
    Trace {
        main: reference_main(&client).await,
        fork: reference_fork(&client).await,
    }
}

/// Differential oracle: the same scenario on both clients gives the same
/// observable result on one server.
#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn client_matches_reference() {
    let _guard = initialize_pool(42, Some(5));
    let (addr, shutdown) = start_server(durable_app());

    let runtime = runtime();
    let reference = runtime.block_on(reference_trace(addr));
    let ours = block_on_future_scoped(our_trace(addr));

    assert_eq!(
        ours, reference,
        "our client and the reference client differ"
    );

    shutdown.turn_on();
}
