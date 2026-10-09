//! End-to-end test for state persistence across a restart (Decision 21, F14).
//!
//! Proves persistence is actually wired into `ProxyServer`: drain a backend via
//! the control socket, stop the proxy, start a fresh one against the same state
//! directory, and confirm the backend comes back up already draining.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use foundation_core::valtron::initialize_pool;
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::{ProxyConfig, ProxyServer};

fn rpc(path: &str, request: &str) -> serde_json::Value {
    let stream = UnixStream::connect(path).expect("connect control socket");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut writer = stream.try_clone().expect("clone stream");
    writeln!(writer, "{request}").expect("write request");
    writer.flush().ok();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read response");
    serde_json::from_str(&line).expect("parse json")
}

#[test]
fn backend_drain_survives_restart() {
    let _guard = initialize_pool(42, Some(4));

    let tag = std::process::id();
    let state_dir = std::env::temp_dir().join(format!("ewe_proxy_persist_{tag}"));
    let _ = std::fs::remove_dir_all(&state_dir);
    let state_dir = state_dir.to_string_lossy().into_owned();
    let sock = std::env::temp_dir()
        .join(format!("ewe_proxy_persist_{tag}.sock"))
        .to_string_lossy()
        .into_owned();

    let build_config = || {
        let svc = ServiceConfig::new("echo", "test.local")
            .backend("http://127.0.0.1:19101")
            .backend("http://127.0.0.1:19102");
        ProxyConfig::new("test.local", "127.0.0.1")
            .bind("127.0.0.1:0")
            .control_socket(&sock)
            .persist_to(&state_dir)
            .service(svc)
    };

    // ── Run 1: drain one backend, then shut down. ──
    {
        let proxy = ProxyServer::start(build_config()).expect("start proxy 1");
        std::thread::sleep(Duration::from_millis(200));

        let resp = rpc(&sock, r#"{"command":"status"}"#);
        assert_eq!(resp["data"]["draining"], 0, "nothing draining at first run: {resp}");

        let resp = rpc(
            &sock,
            r#"{"command":"drain","params":{"service":"echo","url":"http://127.0.0.1:19101"}}"#,
        );
        assert_eq!(resp["ok"], true, "drain ok: {resp}");
        proxy.shutdown();
    }

    // ── Run 2: fresh proxy, same state dir — the drain must be restored. ──
    {
        let proxy = ProxyServer::start(build_config()).expect("start proxy 2");
        std::thread::sleep(Duration::from_millis(200));

        let resp = rpc(&sock, r#"{"command":"status"}"#);
        assert_eq!(
            resp["data"]["draining"], 1,
            "the drained backend must be restored after restart: {resp}"
        );

        // And specifically the right backend.
        let resp = rpc(&sock, r#"{"command":"list"}"#);
        let backends = resp["data"][0]["backends"].as_array().expect("backends array");
        let drained = backends
            .iter()
            .find(|b| b["url"] == "http://127.0.0.1:19101")
            .expect("backend present");
        assert_eq!(drained["state"], "Draining", "restored backend is draining: {resp}");

        proxy.shutdown();
    }

    let _ = std::fs::remove_dir_all(&state_dir);
}
