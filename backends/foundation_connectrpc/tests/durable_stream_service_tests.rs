//! The Durable Streams `ConnectRPC` surface (spec 71 feature 06, task T9).
//!
//! Each test builds the adapter over the in-memory VFS and drives it through
//! the generated client. The client speaks Connect over HTTP/2 cleartext to a
//! loopback server, so the generated wire types are exercised.
//!
//! The tests share the process-wide valtron pool, so they run one at a time.

#![cfg(not(target_family = "wasm"))]

use std::sync::{mpsc, Arc};
use std::time::Duration;

use foundation_connectrpc::shared::durable::proto::v1 as pb;
use foundation_connectrpc::shared::durable::proto::v1::durable_stream_service::{
    procedure, DurableStreamServiceClient,
};
use foundation_connectrpc::shared::durable::{code_for, durable_stream_router, ConnectRpcAdapter};
use foundation_connectrpc::{
    ClientOptions, Code, ConnectRpcServe, ConnectRpcServeH2, H2Transport, Request, RpcCtx,
    Transport,
};
use foundation_context::Ctx;
use foundation_core::durable::{
    wire, BadRequestKind, ContentType, DurableService, DurableStream, MemoryVfs, StoreError,
    StreamName, StreamOffset,
};
use foundation_core::valtron::{block_on_future_scoped, initialize_pool};
use foundation_http::native::app::ServerApp;
use foundation_http::native::durable::{dispatch, DurableRequest};
use foundation_http::native::serve::{H2Serve, Serve};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use foundation_netio::shared::http::{SimpleHeaders, SimpleMethod};
use serial_test::serial;

/// Every procedure path the test server routes.
const PROCEDURES: [&str; 8] = [
    procedure::CREATE,
    procedure::APPEND,
    procedure::READ,
    procedure::SUBSCRIBE,
    procedure::HEAD,
    procedure::CLOSE,
    procedure::DELETE,
    procedure::FORK,
];

/// A listening test server.
struct Running {
    base_url: String,
    shutdown: Arc<ShutdownSignal>,
    done: mpsc::Receiver<()>,
}

impl Running {
    fn stop(self) {
        self.shutdown.turn_on();
        let _ = self.done.recv_timeout(Duration::from_secs(5));
    }
}

/// Start a `ConnectRPC` server for `adapter` on a loopback port.
fn start_server(adapter: Arc<ConnectRpcAdapter>) -> Running {
    let h1: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(
        durable_stream_router(Arc::clone(&adapter)).into_handler(),
    ));
    let h2: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(
        durable_stream_router(adapter).into_handler(),
    ));
    let mut http1 = HttpApp::new_serve();
    let mut http2 = HttpApp::new_h2_serve();
    for path in PROCEDURES {
        http1.router.add_route_any(path, &h1);
        http2.route_any_h2(path, Arc::clone(&h2));
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    listener
        .set_nonblocking(true)
        .expect("set the listener non-blocking");
    let addr = listener.local_addr().expect("local address");
    let server = HttpServer::from_app_with_config(
        ServerApp::both(http1, http2),
        &addr.to_string(),
        ServerConfig::defaults(),
    );
    let shutdown = Arc::new(ShutdownSignal::new());
    let thread_shutdown = Arc::clone(&shutdown);
    let (done_tx, done) = mpsc::channel();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &thread_shutdown);
        let _ = done_tx.send(());
    });

    Running {
        base_url: format!("http://{addr}"),
        shutdown,
        done,
    }
}

/// A service over a fresh in-memory VFS.
fn service() -> Arc<ConnectRpcAdapter> {
    let stream = DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host());
    Arc::new(ConnectRpcAdapter::new(Arc::new(DurableService::new(
        stream,
    ))))
}

/// A generated client that speaks Connect over HTTP/2 cleartext.
fn client(base_url: &str) -> DurableStreamServiceClient {
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
    DurableStreamServiceClient::new(transport, base_url, ClientOptions::new().with_connect())
        .expect("client")
}

/// A client call context with a deadline that outlasts a slow CI worker.
fn ctx() -> RpcCtx {
    RpcCtx::client(Ctx::host()).with_deadline(Duration::from_secs(120))
}

/// The wire text of a byte position.
fn offset(position: u64) -> String {
    wire::offset_text(&StreamOffset::from_bytes(position.to_be_bytes().to_vec()))
}

/// Create a stream with a text content type.
fn create(client: &DurableStreamServiceClient, name: &str) -> pb::CreateResponse {
    block_on_future_scoped(client.create(
        ctx(),
        Request::new(pb::CreateRequest {
            name: name.to_string(),
            content_type: Some("text/plain".to_string()),
            ..Default::default()
        }),
    ))
    .expect("create")
    .msg
}

/// Append one text chunk.
fn append(client: &DurableStreamServiceClient, name: &str, bytes: &[u8]) -> pb::AppendResponse {
    block_on_future_scoped(client.append(
        ctx(),
        Request::new(pb::AppendRequest {
            name: name.to_string(),
            chunk: bytes.to_vec(),
            content_type: Some("text/plain".to_string()),
            ..Default::default()
        }),
    ))
    .expect("append")
    .msg
}

#[test]
#[serial(durable_rpc)]
fn every_operation_round_trips_over_the_generated_client() {
    let _pool = initialize_pool(7101, Some(8));
    let running = start_server(service());
    let client = client(&running.base_url);

    block_on_future_scoped(async {
        let created = create(&client, "orders");
        assert!(created.created, "a new stream is created");
        let head = created.head.as_option().expect("head");
        assert_eq!(head.content_type, "text/plain");
        assert_eq!(head.tail, offset(0));
        assert!(!head.closed);

        let appended = append(&client, "orders", b"alpha");
        assert!(!appended.duplicate);
        let cursor = appended.cursor.as_option().expect("cursor");
        assert_eq!(cursor.next, offset(5));
        assert!(cursor.up_to_date);
        assert!(!cursor.closed);

        let head = client
            .head(
                ctx(),
                Request::new(pb::HeadRequest {
                    name: "orders".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect("head")
            .msg;
        let head = head.head.as_option().expect("head");
        assert_eq!(head.tail, offset(5));

        let page = client
            .read(
                ctx(),
                Request::new(pb::ReadRequest {
                    name: "orders".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect("read")
            .msg;
        assert_eq!(page.chunks.len(), 1);
        assert_eq!(page.chunks[0].data, b"alpha".to_vec());
        assert_eq!(page.cursor.as_option().expect("cursor").next, offset(5));

        let closed = client
            .close(
                ctx(),
                Request::new(pb::CloseRequest {
                    name: "orders".to_string(),
                    final_chunk: Some(b"omega".to_vec()),
                    ..Default::default()
                }),
            )
            .await
            .expect("close")
            .msg;
        assert!(closed.closed);
        let cursor = closed.cursor.as_option().expect("cursor");
        assert!(cursor.closed);
        assert_eq!(cursor.next, offset(10));

        let refused = client
            .append(
                ctx(),
                Request::new(pb::AppendRequest {
                    name: "orders".to_string(),
                    chunk: b"late".to_vec(),
                    content_type: Some("text/plain".to_string()),
                    ..Default::default()
                }),
            )
            .await
            .expect_err("append after close");
        assert_eq!(refused.current_context().code(), Code::FailedPrecondition);

        let forked = client
            .fork(
                ctx(),
                Request::new(pb::ForkRequest {
                    name: "orders-copy".to_string(),
                    source: "orders".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect("fork")
            .msg;
        assert!(forked.created);
        let head = forked.head.as_option().expect("head");
        assert_eq!(head.content_type, "text/plain");
        assert_eq!(head.tail, offset(10));

        let page = client
            .read(
                ctx(),
                Request::new(pb::ReadRequest {
                    name: "orders-copy".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect("read the fork")
            .msg;
        let bytes: Vec<u8> = page
            .chunks
            .iter()
            .flat_map(|chunk| chunk.data.clone())
            .collect();
        assert_eq!(bytes, b"alphaomega".to_vec());

        let deleted = client
            .delete(
                ctx(),
                Request::new(pb::DeleteRequest {
                    name: "orders".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect("delete")
            .msg;
        assert!(deleted.soft_deleted, "a fork keeps the source data");

        let missing = client
            .head(
                ctx(),
                Request::new(pb::HeadRequest {
                    name: "orders".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect_err("the source is gone");
        assert_eq!(missing.current_context().code(), Code::NotFound);

        let deleted = client
            .delete(
                ctx(),
                Request::new(pb::DeleteRequest {
                    name: "orders-copy".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect("delete the fork")
            .msg;
        assert!(!deleted.soft_deleted);
    });

    running.stop();
}

#[test]
#[serial(durable_rpc)]
fn refusals_map_to_connect_codes_over_the_wire() {
    let _pool = initialize_pool(7102, Some(8));
    let running = start_server(service());
    let client = client(&running.base_url);

    block_on_future_scoped(async {
        let missing = client
            .append(
                ctx(),
                Request::new(pb::AppendRequest {
                    name: "nobody".to_string(),
                    chunk: b"x".to_vec(),
                    content_type: Some("text/plain".to_string()),
                    ..Default::default()
                }),
            )
            .await
            .expect_err("no such stream");
        assert_eq!(missing.current_context().code(), Code::NotFound);

        let bad_name = client
            .create(
                ctx(),
                Request::new(pb::CreateRequest {
                    name: "../escape".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect_err("an invalid path");
        assert_eq!(bad_name.current_context().code(), Code::InvalidArgument);

        create(&client, "orders");

        let empty = client
            .append(
                ctx(),
                Request::new(pb::AppendRequest {
                    name: "orders".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect_err("an empty chunk is not a close");
        assert_eq!(empty.current_context().code(), Code::InvalidArgument);

        let mismatch = client
            .append(
                ctx(),
                Request::new(pb::AppendRequest {
                    name: "orders".to_string(),
                    chunk: b"x".to_vec(),
                    content_type: Some("application/json".to_string()),
                    ..Default::default()
                }),
            )
            .await
            .expect_err("the content types differ");
        assert_eq!(mismatch.current_context().code(), Code::FailedPrecondition);

        let conflict = client
            .create(
                ctx(),
                Request::new(pb::CreateRequest {
                    name: "orders".to_string(),
                    content_type: Some("application/json".to_string()),
                    ..Default::default()
                }),
            )
            .await
            .expect_err("a different configuration");
        assert_eq!(conflict.current_context().code(), Code::AlreadyExists);

        client
            .append(
                ctx(),
                Request::new(pb::AppendRequest {
                    name: "orders".to_string(),
                    chunk: b"second".to_vec(),
                    content_type: Some("text/plain".to_string()),
                    stream_seq: Some("b".to_string()),
                    ..Default::default()
                }),
            )
            .await
            .expect("first sequence");
        let regression = client
            .append(
                ctx(),
                Request::new(pb::AppendRequest {
                    name: "orders".to_string(),
                    chunk: b"first".to_vec(),
                    content_type: Some("text/plain".to_string()),
                    stream_seq: Some("a".to_string()),
                    ..Default::default()
                }),
            )
            .await
            .expect_err("the sequence goes backwards");
        assert_eq!(regression.current_context().code(), Code::Aborted);
    });

    running.stop();
}

#[test]
fn the_status_table_covers_every_core_refusal() {
    let cases: Vec<(StoreError, Code)> = vec![
        (StoreError::NotFound, Code::NotFound),
        (StoreError::Gone, Code::NotFound),
        (StoreError::Trimmed, Code::OutOfRange),
        (
            StoreError::Fenced { current_epoch: 2 },
            Code::PermissionDenied,
        ),
        (
            StoreError::InvalidEpochSeq { seq: 3 },
            Code::InvalidArgument,
        ),
        (
            StoreError::SequenceGap {
                expected: 1,
                received: 2,
            },
            Code::Aborted,
        ),
        (StoreError::StreamSeqConflict { last: None }, Code::Aborted),
        (
            StoreError::ContentTypeMismatch {
                expected: ContentType::default(),
                got: ContentType::parse("text/plain").expect("content type"),
            },
            Code::FailedPrecondition,
        ),
        (StoreError::ConfigConflict, Code::AlreadyExists),
        (
            StoreError::Closed {
                tail: StreamOffset::from_bytes(0u64.to_be_bytes().to_vec()),
            },
            Code::FailedPrecondition,
        ),
        (
            StoreError::BadRequest(BadRequestKind::Body),
            Code::InvalidArgument,
        ),
        (
            StoreError::RateLimited { retry_after: None },
            Code::ResourceExhausted,
        ),
        (StoreError::Unsupported, Code::Unimplemented),
        (StoreError::Storage(Box::new(TestStorage)), Code::Internal),
    ];
    for (error, code) in cases {
        assert_eq!(code_for(&error), code, "{error} maps to {code:?}");
    }
}

/// A stand-in backend failure for the status table.
#[derive(Debug)]
struct TestStorage;

impl std::fmt::Display for TestStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("test storage failure")
    }
}

impl std::error::Error for TestStorage {}

#[test]
#[serial(durable_rpc)]
fn subscribe_delivers_an_append_after_the_stream_opens() {
    let _pool = initialize_pool(7103, Some(8));
    let running = start_server(service());
    let client = client(&running.base_url);

    block_on_future_scoped(async {
        create(&client, "live");

        let mut stream = client
            .subscribe(
                ctx(),
                Request::new(pb::SubscribeRequest {
                    name: "live".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect("subscribe");

        let first = stream
            .receive()
            .await
            .expect("receive")
            .expect("the open page");
        assert!(first.chunks.is_empty(), "the stream starts at the tail");
        let cursor = first.cursor.as_option().expect("cursor");
        assert_eq!(cursor.next, offset(0));
        assert!(cursor.up_to_date);
        assert!(!cursor.closed);

        append(&client, "live", b"live-1");

        let page = stream
            .receive()
            .await
            .expect("receive")
            .expect("the appended page");
        assert_eq!(page.chunks.len(), 1);
        assert_eq!(page.chunks[0].data, b"live-1".to_vec());
        assert_eq!(page.cursor.as_option().expect("cursor").next, offset(6));

        client
            .close(
                ctx(),
                Request::new(pb::CloseRequest {
                    name: "live".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect("close");

        let final_page = stream
            .receive()
            .await
            .expect("receive")
            .expect("the final page");
        assert!(final_page.cursor.as_option().expect("cursor").closed);
        assert!(
            stream.receive().await.expect("end").is_none(),
            "the stream ends after the close"
        );
    });

    running.stop();
}

#[test]
#[serial(durable_rpc)]
fn rpc_stream_resumes_from_offset() {
    let _pool = initialize_pool(7104, Some(8));
    let running = start_server(service());
    let client = client(&running.base_url);

    block_on_future_scoped(async {
        create(&client, "resume");
        append(&client, "resume", b"a");
        append(&client, "resume", b"b");

        let mut stream = client
            .subscribe(
                ctx(),
                Request::new(pb::SubscribeRequest {
                    name: "resume".to_string(),
                    offset: Some(offset(1)),
                    ..Default::default()
                }),
            )
            .await
            .expect("subscribe");

        let page = stream
            .receive()
            .await
            .expect("receive")
            .expect("the backlog page");
        let bytes: Vec<u8> = page
            .chunks
            .iter()
            .flat_map(|chunk| chunk.data.clone())
            .collect();
        assert_eq!(
            bytes,
            b"b".to_vec(),
            "the stream resumes after the first chunk"
        );

        append(&client, "resume", b"c");

        let page = stream
            .receive()
            .await
            .expect("receive")
            .expect("the live page");
        assert_eq!(page.chunks.len(), 1);
        assert_eq!(page.chunks[0].data, b"c".to_vec());
    });

    running.stop();
}

#[test]
#[serial(durable_rpc)]
fn http_and_rpc_return_same_chunks() {
    let _pool = initialize_pool(7105, Some(8));
    let stream = DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host());
    let protocol = Arc::new(DurableService::new(stream));
    let running = start_server(Arc::new(ConnectRpcAdapter::new(Arc::clone(&protocol))));
    let client = client(&running.base_url);

    block_on_future_scoped(async {
        create(&client, "doc");
        append(&client, "doc", b"part-one;");
        append(&client, "doc", b"part-two");

        let page = client
            .read(
                ctx(),
                Request::new(pb::ReadRequest {
                    name: "doc".to_string(),
                    ..Default::default()
                }),
            )
            .await
            .expect("rpc read")
            .msg;
        let rpc_bytes: Vec<u8> = page
            .chunks
            .iter()
            .flat_map(|chunk| chunk.data.clone())
            .collect();

        let response = dispatch(
            &protocol,
            &Ctx::host(),
            DurableRequest {
                method: SimpleMethod::GET,
                name: StreamName::parse("doc").expect("a valid name"),
                headers: SimpleHeaders::new(),
                queries: None,
                body: Vec::new(),
            },
        )
        .await
        .expect("http read");
        assert_eq!(response.status, 200);
        assert_eq!(rpc_bytes, response.body);
    });

    running.stop();
}
