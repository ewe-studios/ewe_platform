//! End-to-end test for the Unix-domain control socket (Decision 20, F16).
//!
//! Proves the admin interface is actually wired into `ProxyServer`: start a
//! proxy with a control socket, connect over the socket, drive JSON-RPC
//! commands, and verify they act on the shared state.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use foundation_core::valtron::initialize_pool;
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::{ProxyConfig, ProxyServer};

/// Send one JSON-RPC line to the control socket and return the parsed response.
fn rpc(path: &str, request: &str) -> serde_json::Value {
    let stream = UnixStream::connect(path).expect("connect control socket");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut writer = stream.try_clone().expect("clone stream");
    writeln!(writer, "{request}").expect("write request");
    writer.flush().ok();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read response");
    serde_json::from_str(&line).expect("parse response json")
}

fn unique_socket_path(tag: &str) -> String {
    let dir = std::env::temp_dir();
    dir.join(format!("ewe_proxy_ctl_{tag}_{}.sock", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

#[test]
fn control_socket_status_and_drain() {
    let _guard = initialize_pool(42, Some(4));
    let sock = unique_socket_path("status");

    let svc = ServiceConfig::new("echo", "test.local")
        .backend("http://127.0.0.1:19001")
        .backend("http://127.0.0.1:19002");
    let config = ProxyConfig::new("test.local", "127.0.0.1")
        .bind("127.0.0.1:0")
        .control_socket(&sock)
        .service(svc);
    let proxy = ProxyServer::start(config).expect("start proxy");

    // Give the control thread a moment to bind and start accepting.
    std::thread::sleep(Duration::from_millis(200));

    // `status` — two backends, none draining yet.
    let resp = rpc(&sock, r#"{"command":"status"}"#);
    assert_eq!(resp["ok"], true, "status ok: {resp}");
    assert_eq!(resp["data"]["total"], 2, "two backends total: {resp}");
    assert_eq!(resp["data"]["draining"], 0, "none draining yet: {resp}");

    // `list` — the service and its backends are visible.
    let resp = rpc(&sock, r#"{"command":"list"}"#);
    assert_eq!(resp["ok"], true, "list ok: {resp}");
    assert_eq!(resp["data"][0]["name"], "echo", "service name: {resp}");
    assert_eq!(resp["data"][0]["backends"].as_array().map(|a| a.len()), Some(2));

    // `drain` one backend, then confirm `status` reflects it.
    let resp = rpc(
        &sock,
        r#"{"command":"drain","params":{"service":"echo","url":"http://127.0.0.1:19001"}}"#,
    );
    assert_eq!(resp["ok"], true, "drain ok: {resp}");

    let resp = rpc(&sock, r#"{"command":"status"}"#);
    assert_eq!(resp["data"]["draining"], 1, "one backend draining now: {resp}");

    // Unknown command is a clean error, not a crash.
    let resp = rpc(&sock, r#"{"command":"bogus"}"#);
    assert_eq!(resp["ok"], false, "unknown command rejected: {resp}");

    proxy.shutdown();

    // The socket file is removed on shutdown.
    assert!(!std::path::Path::new(&sock).exists(), "socket file cleaned up");
}
