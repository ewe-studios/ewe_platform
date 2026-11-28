//! The connection contract of the JSON-RPC engine (spec 88 invariants I3, I6).
//!
//! A fake WebSocket endpoint accepts the upgrade and then closes the socket or
//! stays silent. No browser and no network beyond the loopback are needed.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use foundation_browser::{BrowserError, RpcEngine, RpcEngineConfig, WireProtocol};
use foundation_netio::websocket::{Opcode, WebSocketFrame};
use serde_json::json;

/// Start a handshaking endpoint and hand the open socket to `behaviour`.
fn endpoint(behaviour: impl FnOnce(TcpStream) + Send + 'static) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("read the address");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept the client");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("bound the fixture reads");
        let key = {
            let mut headers = BufReader::new(Read::by_ref(&mut stream).take(4096));
            let mut line = String::new();
            let mut key = None;
            loop {
                line.clear();
                let read = headers
                    .read_line(&mut line)
                    .expect("read an upgrade header");
                assert_ne!(read, 0, "the client sends the complete upgrade header");
                if line == "\r\n" {
                    break;
                }
                if let Some((_, value)) = line
                    .split_once(':')
                    .filter(|(name, _)| name.eq_ignore_ascii_case("Sec-WebSocket-Key"))
                {
                    key = Some(value.trim().to_owned());
                }
            }
            key.expect("the client sends a WebSocket key")
        };
        let accept = foundation_netio::websocket::compute_accept_key(&key);
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
    (format!("ws://{address}/"), worker)
}

#[test]
fn a_closed_connection_fails_the_next_call_at_once() {
    let (url, worker) = endpoint(|mut stream| {
        // Read the first command frame, then close the socket.
        let frame = WebSocketFrame::decode(&mut stream).expect("read the first command frame");
        assert_eq!(frame.opcode, Opcode::Text);
        assert!(frame.mask.is_some(), "the client masks the command frame");
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
                worker
                    .join()
                    .expect("the upgrade and command reached the endpoint");
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
    let (url, worker) = endpoint(|_stream| std::thread::sleep(Duration::from_secs(2)));
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
    worker.join().expect("the endpoint accepted the upgrade");
}
