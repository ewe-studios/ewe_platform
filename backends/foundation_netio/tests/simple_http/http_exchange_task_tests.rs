//! Tests for `HttpExchangeTask` — validate it yields `Head` + body chunks for
//! a real HTTP request, and that mapped output flows into pipes.

use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::url::Uri;
use foundation_core::valtron::{self, Stream, TaskIteratorExt, TaskStatus};
use foundation_netio::simple_http::client::shared::request_task::{
    HttpExchange, HttpExchangePending,
};
use foundation_netio::simple_http::client::shared::{ClientConfig, PreparedRequest, SystemDnsResolver};
use foundation_netio::simple_http::client::{HttpConnectionPool, HttpExchangeTask};
use foundation_netio::simple_http::shared::{
    pushable_request_body_with_depth, Extensions, SimpleHeaders, SimpleMethod, Status,
    DEFAULT_PUSHABLE_DEPTH,
};

/// Raw TCP server that reads an HTTP/1.1 request and responds with `status + body`.
/// Returns the listening address so clients can connect.
fn raw_http_serve(status: u16, body: &'static str) -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let body_bytes = body.as_bytes().to_vec();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.expect("accept");
            let mut buf = [0u8; 4096];
            let _n = stream.read(&mut buf).unwrap_or(0);
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body_bytes.len()
            );
            let mut out = response.into_bytes();
            out.extend_from_slice(&body_bytes);
            let _ = stream.write_all(&out);
            break;
        }
    });
    addr
}

/// Build a PreparedRequest for a given URL + method.
fn request_to(method: SimpleMethod, url: &str) -> PreparedRequest {
    let (pushable, body_stream) = pushable_request_body_with_depth(DEFAULT_PUSHABLE_DEPTH);
    let _sender = pushable.into_sender();
    PreparedRequest {
        method,
        url: Uri::parse(url).expect("uri"),
        headers: {
            let mut h = SimpleHeaders::new();
            h.insert(
                foundation_netio::simple_http::shared::SimpleHeader::HOST,
                vec!["127.0.0.1".to_string()],
            );
            h
        },
        body: body_stream,
        extensions: Extensions::new(),
    }
}

fn default_pool_and_config() -> (Arc<HttpConnectionPool<SystemDnsResolver>>, ClientConfig) {
    (Arc::new(HttpConnectionPool::default()), ClientConfig::default())
}

/// Test: `HttpExchangeTask` yields `Head` + `BodyChunk` for a 200 response.
#[test]
fn yields_head_then_body_then_exhausts() {
    let _guard = foundation_core::valtron::initialize_pool(80, Some(4));
    let addr = raw_http_serve(200, "hello world");
    let url = format!("http://127.0.0.1:{}/test", addr.port());
    let (pool, config) = default_pool_and_config();

    let task = HttpExchangeTask::new(request_to(SimpleMethod::GET, &url), config.max_redirects, pool, config);
    let mut stream = valtron::execute(task, None).expect("execute");

    let mut saw_head = false;
    let mut body_bytes = Vec::new();

    while let Some(item) = stream.next() {
        match item {
            Stream::Next(HttpExchange::Head { status, .. }) => {
                assert_eq!(status, Status::OK);
                saw_head = true;
            }
            Stream::Next(HttpExchange::BodyChunk(b)) => {
                body_bytes.extend_from_slice(&b);
            }
            Stream::Next(HttpExchange::Failed(e)) => panic!("unexpected Failed: {e}"),
            _ => continue,
        }
    }

    assert!(saw_head, "should have seen Head");
    assert!(!body_bytes.is_empty(), "should have received body bytes");
    assert_eq!(String::from_utf8_lossy(&body_bytes), "hello world");
}

/// Test: `HttpExchangeTask` yields `Failed` for a closed port.
#[test]
fn yields_failed_for_connection_refused() {
    let _guard = foundation_core::valtron::initialize_pool(81, Some(4));
    let (pool, config) = default_pool_and_config();

    let task = HttpExchangeTask::new(
        request_to(SimpleMethod::GET, "http://127.0.0.1:1/nothing"),
        config.max_redirects,
        pool,
        config,
    );
    let mut stream = valtron::execute(task, None).expect("execute");

    let mut saw_failed = false;
    while let Some(item) = stream.next() {
        if let Stream::Next(HttpExchange::Failed(_)) = item {
            saw_failed = true;
            break;
        }
    }
    assert!(saw_failed, "should have seen Failed for dead server");
}

/// Test: `map_ready` forwards `Head` and `BodyChunk` into pipes end-to-end.
#[test]
fn map_ready_into_pipes() {
    use foundation_core::valtron::TryRecvError;

    let _guard = foundation_core::valtron::initialize_pool(82, Some(4));
    let addr = raw_http_serve(200, "piped response body");
    let url = format!("http://127.0.0.1:{}/test", addr.port());
    let (pool, config) = default_pool_and_config();

    let (head_tx, head_rx): (
        foundation_core::valtron::PipeSender<(Status, SimpleHeaders)>,
        foundation_core::valtron::PipeReceiver<(Status, SimpleHeaders)>,
    ) = foundation_core::valtron::Pipe::with_depth(1);
    let (recv_tx, recv_body): (
        foundation_core::valtron::PipeSender<Bytes>,
        foundation_core::valtron::PipeReceiver<Bytes>,
    ) = foundation_core::valtron::Pipe::with_depth(DEFAULT_PUSHABLE_DEPTH);

    let task = HttpExchangeTask::new(
        request_to(SimpleMethod::GET, &url),
        config.max_redirects,
        pool,
        config,
    )
    .map_ready(move |item| match item {
        HttpExchange::Head { status, headers } => {
            let _ = head_tx.try_send((status, headers));
        }
        HttpExchange::BodyChunk(bytes) => {
            let _ = recv_tx.try_send(bytes);
        }
        HttpExchange::Failed(_) => {}
    });

    // Drive to completion.
    let mut stream = valtron::execute(task, None).expect("execute");
    while stream.next().is_some() {}

    // Verify head pipe.
    match head_rx.try_recv() {
        Ok((status, _)) => assert_eq!(status, Status::OK),
        Err(TryRecvError::Closed) => panic!("head pipe closed without data"),
        Err(TryRecvError::Empty) => panic!("head pipe empty"),
    }

    // Verify body pipe.
    let mut body = Vec::new();
    loop {
        match recv_body.try_recv() {
            Ok(b) => body.extend_from_slice(&b),
            Err(TryRecvError::Closed | TryRecvError::Empty) => break,
        }
    }
    assert!(!body.is_empty(), "should have received body bytes");
    assert_eq!(String::from_utf8_lossy(&body), "piped response body");
}

/// Test: `map_ready` closure runs on Head + BodyChunk, pipes receive data.
#[test]
fn map_ready_receives_both_head_and_body() {
    use foundation_core::valtron::TryRecvError;
    use std::sync::atomic::{AtomicBool, Ordering};

    let _guard = foundation_core::valtron::initialize_pool(83, Some(4));
    let addr = raw_http_serve(200, "map ready data");
    let url = format!("http://127.0.0.1:{}/test", addr.port());
    let (pool, config) = default_pool_and_config();

    let got_head = Arc::new(AtomicBool::new(false));
    let got_body = Arc::new(AtomicBool::new(false));
    let gh = got_head.clone();
    let gb = got_body.clone();

    let (head_tx, head_rx): (
        foundation_core::valtron::PipeSender<(Status, SimpleHeaders)>,
        foundation_core::valtron::PipeReceiver<(Status, SimpleHeaders)>,
    ) = foundation_core::valtron::Pipe::with_depth(1);
    let (recv_tx, recv_body): (
        foundation_core::valtron::PipeSender<Bytes>,
        foundation_core::valtron::PipeReceiver<Bytes>,
    ) = foundation_core::valtron::Pipe::with_depth(DEFAULT_PUSHABLE_DEPTH);

    let task = HttpExchangeTask::new(
        request_to(SimpleMethod::GET, &url),
        config.max_redirects,
        pool,
        config,
    )
    .map_ready(move |item| match item {
        HttpExchange::Head { status, headers } => {
            gh.store(true, Ordering::SeqCst);
            let _ = head_tx.try_send((status, headers));
        }
        HttpExchange::BodyChunk(bytes) => {
            gb.store(true, Ordering::SeqCst);
            let _ = recv_tx.try_send(bytes);
        }
        HttpExchange::Failed(_) => {}
    });

    let mut stream = valtron::execute(task, None).expect("execute");
    while stream.next().is_some() {}

    assert!(got_head.load(Ordering::SeqCst), "map_ready should have received Head");
    assert!(got_body.load(Ordering::SeqCst), "map_ready should have received BodyChunk");

    // Verify data from pipes.
    let (status, _) = head_rx.try_recv().expect("head data");
    assert_eq!(status, Status::OK);
    let b = recv_body.try_recv().expect("body data");
    assert_eq!(String::from_utf8_lossy(&b), "map ready data");
}
