//! The connection contract of the JSON-RPC engine (spec 88 invariants I3, I6).
//!
//! A fake WebSocket endpoint accepts the upgrade and then closes the socket or
//! stays silent. No browser and no network beyond the loopback are needed.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use foundation_browser::{BrowserError, RpcEngine, RpcEngineConfig, WireProtocol};
use serde_json::json;

/// Start a handshaking endpoint and hand the open socket to `behaviour`.
fn endpoint(behaviour: impl FnOnce(TcpStream) + Send + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("read the address");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept the client");
        let mut request = [0u8; 4096];
        let read = stream.read(&mut request).expect("read the upgrade request");
        let request = String::from_utf8_lossy(&request[..read]).to_string();
        let key = request
            .lines()
            .find_map(|line| line.strip_prefix("Sec-WebSocket-Key:"))
            .expect("the client sends a WebSocket key")
            .trim();
        let accept = foundation_netio::websocket::compute_accept_key(key);
        write!(
            stream,
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {accept}\r\n\r\n"
        )
        .expect("write the upgrade response");
        stream.flush().expect("flush the upgrade response");
        behaviour(stream);
    });
    format!("ws://{address}/")
}

#[test]
fn a_closed_connection_fails_the_next_call_at_once() {
    let url = endpoint(|mut stream| {
        // Read the first command frame, then close the socket.
        let mut frame = [0u8; 1024];
        let _ = stream.read(&mut frame);
    });
    let engine = RpcEngine::connect(
        &url,
        RpcEngineConfig::default().with_call_timeout(Duration::from_secs(2)),
    )
    .expect("connect to the endpoint");

    // The reader notices the close on its own schedule. Retry until it does,
    // with a deadline so a broken engine fails the test instead of hanging it.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let started = Instant::now();
        match engine.call("Page.navigate", json!({ "url": "about:blank" }), None) {
            Err(BrowserError::ConnectionLost { reason }) => {
                assert!(
                    reason.contains("close")
                        || reason.contains("ended")
                        || reason.contains("read error")
                        || reason.contains("reset")
                        || reason.contains("eof")
                        || reason.contains("broken pipe"),
                    "the reason must name what ended the reader: {reason}"
                );
                // The bound proves that the call does not wait for its 2 s
                // deadline. It leaves room for a loaded continuous integration
                // (CI) runner to schedule the reader's notification.
                assert!(
                    started.elapsed() < Duration::from_millis(1500),
                    "the call must not wait for its deadline: {:?}",
                    started.elapsed()
                );
                return;
            }
            Err(BrowserError::Timeout { .. }) if Instant::now() < deadline => {}
            other => panic!("expected ConnectionLost, got {other:?}"),
        }
    }
}

#[test]
fn a_call_timeout_comes_from_the_configuration() {
    // Keep the socket open and silent: the call can only end on its deadline.
    let url = endpoint(|_stream| std::thread::sleep(Duration::from_secs(2)));
    let engine = RpcEngine::connect(
        &url,
        RpcEngineConfig::default().with_call_timeout(Duration::from_millis(150)),
    )
    .expect("connect to the endpoint");

    let error = engine
        .call("Page.enable", json!({}), None)
        .expect_err("a silent endpoint never answers");
    match error {
        BrowserError::Timeout { op, after_ms } => {
            assert_eq!(op, "Page.enable");
            assert_eq!(after_ms, 150, "the error names the configured value");
        }
        other => panic!("expected Timeout, got {other:?}"),
    }
}
