//! End-to-end proxy data-plane tests against real backend containers.
//!
//! WHY: Unit tests prove the router, balancer, and header logic in isolation.
//! These prove the assembled proxy actually relays bytes: a request through the
//! front end reaches a backend and the response comes back intact, round-robin
//! spreads load, host/path routing selects the right backend, health probes
//! eject and readmit backends, `503`/`404` are answered correctly, hop-by-hop
//! headers are stripped, `X-Forwarded-For` is appended, and `tcp://` passthrough
//! round-trips raw bytes.
//!
//! WHAT: Backends are real containers via the spec-53 docker testbed
//! (`foundation_deployment_platform`). `hashicorp/http-echo` returns a fixed
//! identifying string (so round-robin and routing are observable);
//! `kennethreitz/httpbin` echoes request headers (so header rewriting is
//! observable).
//!
//! HOW: Container-backed tests are `#[ignore]` (they need a Docker daemon) and
//! guarded by `docker_available()`. The proxy needs a valtron pool, so each test
//! initialises one and holds the guard. The test client speaks raw HTTP/1.1 over
//! `std::net::TcpStream` — no HTTP-client dependency, and the proxy answers
//! `Connection: close` so `read_to_end` terminates cleanly.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use foundation_core::valtron::initialize_pool;
use foundation_deployment_platform::docker::{ContainerConfig, ContainerHandle, WaitFor};
use foundation_proxy::config::HealthCheckConfig;
use foundation_proxy::{BackendTarget, ProxyConfig, ProxyServer, ServiceConfig, TcpPassthrough};

/// Shared tokio runtime for container lifecycle (bollard is async).
static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
});

const ECHO_PORT: u16 = 5678;
const HTTPBIN_PORT: u16 = 80;

fn docker_available() -> bool {
    std::path::Path::new("/var/run/docker.sock").exists()
}

/// An `http-echo` container that returns `text` for every request.
fn echo_config(text: &str) -> ContainerConfig {
    ContainerConfig::new("hashicorp/http-echo:latest")
        .command(vec![format!("-text={text}"), format!("-listen=:{ECHO_PORT}")])
        .port(ECHO_PORT)
        .wait(WaitFor::port_with_timeout(ECHO_PORT, Duration::from_secs(60)))
        .stop_timeout_secs(2)
}

/// An `http-echo` container bound to a fixed host port (for restart-in-place).
fn echo_config_fixed(text: &str, host_port: u16) -> ContainerConfig {
    ContainerConfig::new("hashicorp/http-echo:latest")
        .command(vec![format!("-text={text}"), format!("-listen=:{ECHO_PORT}")])
        .port_mapped(ECHO_PORT, host_port)
        .wait(WaitFor::port_with_timeout(ECHO_PORT, Duration::from_secs(60)))
        .stop_timeout_secs(2)
}

/// A `httpbin` container that echoes request headers at `/headers`.
fn httpbin_config() -> ContainerConfig {
    ContainerConfig::new("kennethreitz/httpbin:latest")
        .port(HTTPBIN_PORT)
        .wait(WaitFor::port_with_timeout(HTTPBIN_PORT, Duration::from_secs(60)))
        .stop_timeout_secs(2)
}

/// A parsed raw HTTP response.
struct RawResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl RawResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Send a raw HTTP/1.1 request through the proxy and read the whole response.
fn send_request(
    proxy_addr: std::net::SocketAddr,
    method: &str,
    path: &str,
    host: &str,
    extra_headers: &[(&str, &str)],
) -> RawResponse {
    let mut stream = TcpStream::connect(proxy_addr).expect("connect proxy");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("set timeout");

    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\n");
    for (k, v) in extra_headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("Connection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).expect("write request");
    stream.flush().expect("flush");

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read response");
    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> RawResponse {
    let text = String::from_utf8_lossy(raw);
    let split = text.find("\r\n\r\n").expect("response has header/body split");
    let head = &text[..split];
    let body = text[split + 4..].to_string();

    let mut lines = head.split("\r\n");
    let status_line = lines.next().expect("status line");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .expect("status code");

    let headers = lines
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
        .collect();

    RawResponse {
        status,
        headers,
        body,
    }
}

/// Build and start a proxy on an ephemeral port with the given services.
///
/// Async because every caller is already inside `RT.block_on`; nesting a second
/// `block_on` on a runtime worker thread panics.
async fn start_proxy(services: Vec<ServiceConfig>) -> ProxyServer {
    let mut config = ProxyConfig::new("test.local", "127.0.0.1").bind("127.0.0.1:0");
    config.services = services;
    config.start().await.expect("start proxy")
}

fn backend_url(handle: &ContainerHandle, container_port: u16) -> String {
    let port = handle.host_port(container_port).expect("mapped port");
    format!("http://127.0.0.1:{port}")
}

/// WHY: The proxy must actually relay — a request through the front end reaches
/// the backend and the response returns intact.
/// WHAT: A GET through the proxy to one http-echo backend returns 200 and the
/// backend's identifying body.
#[test]
#[ignore = "requires Docker daemon"]
fn test_forward_roundtrips_to_backend() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    RT.block_on(async {
        let backend = ContainerHandle::start_async(echo_config("backend-solo"))
            .await
            .expect("start echo");
        let url = backend_url(&backend, ECHO_PORT);

        let proxy = start_proxy(vec![
            ServiceConfig::new("app", "app.local").backend(&url)
        ]).await;
        let addr = proxy.local_addr();

        let resp = send_request(addr, "GET", "/", "app.local", &[]);
        assert_eq!(resp.status, 200, "expected 200 from backend");
        assert!(
            resp.body.contains("backend-solo"),
            "body should carry backend identity, got: {:?}",
            resp.body
        );

        proxy.shutdown();
        backend.shutdown_async().await.ok();
    });
}

/// WHY: Weighted round-robin must spread traffic across healthy backends.
/// WHAT: With two equal-weight backends, repeated requests hit both.
#[test]
#[ignore = "requires Docker daemon"]
fn test_round_robin_spreads_across_backends() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    RT.block_on(async {
        let a = ContainerHandle::start_async(echo_config("backend-A"))
            .await
            .expect("start A");
        let b = ContainerHandle::start_async(echo_config("backend-B"))
            .await
            .expect("start B");

        let proxy = start_proxy(vec![ServiceConfig::new("app", "app.local")
            .backend(&backend_url(&a, ECHO_PORT))
            .backend(&backend_url(&b, ECHO_PORT))]).await;
        let addr = proxy.local_addr();

        let mut saw_a = false;
        let mut saw_b = false;
        for _ in 0..10 {
            let resp = send_request(addr, "GET", "/", "app.local", &[]);
            if resp.body.contains("backend-A") {
                saw_a = true;
            }
            if resp.body.contains("backend-B") {
                saw_b = true;
            }
        }
        assert!(saw_a && saw_b, "round-robin must hit both backends (A={saw_a}, B={saw_b})");

        proxy.shutdown();
        a.shutdown_async().await.ok();
        b.shutdown_async().await.ok();
    });
}

/// WHY: Host routing sends different hosts to different services; an unknown
/// host is a 404, not a panic.
/// WHAT: Two hosts map to two backends; a third host returns 404.
#[test]
#[ignore = "requires Docker daemon"]
fn test_host_routing_and_unknown_host_404() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    RT.block_on(async {
        let a = ContainerHandle::start_async(echo_config("host-A"))
            .await
            .expect("start A");
        let b = ContainerHandle::start_async(echo_config("host-B"))
            .await
            .expect("start B");

        let proxy = start_proxy(vec![
            ServiceConfig::new("a", "a.local").backend(&backend_url(&a, ECHO_PORT)),
            ServiceConfig::new("b", "b.local").backend(&backend_url(&b, ECHO_PORT)),
        ]).await;
        let addr = proxy.local_addr();

        let ra = send_request(addr, "GET", "/", "a.local", &[]);
        assert!(ra.body.contains("host-A"), "a.local → host-A, got {:?}", ra.body);
        let rb = send_request(addr, "GET", "/", "b.local", &[]);
        assert!(rb.body.contains("host-B"), "b.local → host-B, got {:?}", rb.body);

        let unknown = send_request(addr, "GET", "/", "nope.local", &[]);
        assert_eq!(unknown.status, 404, "unknown host must 404");

        proxy.shutdown();
        a.shutdown_async().await.ok();
        b.shutdown_async().await.ok();
    });
}

/// WHY: The router picks the longest matching path prefix.
/// WHAT: Same host, prefixes `/` and `/api`; `/api/x` reaches the `/api` backend
/// and `/` reaches the root backend.
#[test]
#[ignore = "requires Docker daemon"]
fn test_path_prefix_longest_wins() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    RT.block_on(async {
        let root = ContainerHandle::start_async(echo_config("root-backend"))
            .await
            .expect("start root");
        let api = ContainerHandle::start_async(echo_config("api-backend"))
            .await
            .expect("start api");

        let proxy = start_proxy(vec![
            ServiceConfig::new("root", "app.local").backend(&backend_url(&root, ECHO_PORT)),
            ServiceConfig::new("api", "app.local")
                .path_prefix("/api")
                .backend(&backend_url(&api, ECHO_PORT)),
        ]).await;
        let addr = proxy.local_addr();

        let r = send_request(addr, "GET", "/api/thing", "app.local", &[]);
        assert!(r.body.contains("api-backend"), "/api/* → api backend, got {:?}", r.body);
        let root_resp = send_request(addr, "GET", "/other", "app.local", &[]);
        assert!(
            root_resp.body.contains("root-backend"),
            "/other → root backend, got {:?}",
            root_resp.body
        );

        proxy.shutdown();
        root.shutdown_async().await.ok();
        api.shutdown_async().await.ok();
    });
}

/// WHY: No available backend must answer 503, never hang or panic.
/// WHAT: A service whose only backend is down (and health-checked) returns 503.
#[test]
#[ignore = "requires Docker daemon"]
fn test_no_healthy_backend_returns_503() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    RT.block_on(async {
        // A backend that is up, then stopped, with a fast health check so it
        // leaves rotation quickly.
        let backend = ContainerHandle::start_async(echo_config("doomed"))
            .await
            .expect("start backend");
        let url = backend_url(&backend, ECHO_PORT);

        let hc = HealthCheckConfig {
            path: "/".into(),
            interval: Duration::from_millis(300),
            timeout: Duration::from_secs(1),
            healthy_threshold: 1,
            unhealthy_threshold: 2,
        };
        let proxy = start_proxy(vec![ServiceConfig::new("app", "app.local")
            .backend(&url)
            .health_check_config(hc)]).await;
        let addr = proxy.local_addr();

        // Kill the backend and wait for the probes to eject it.
        backend.shutdown_async().await.ok();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut got_503 = false;
        while Instant::now() < deadline {
            let resp = send_request(addr, "GET", "/", "app.local", &[]);
            if resp.status == 503 {
                got_503 = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        assert!(got_503, "expected 503 once the only backend is unhealthy");

        proxy.shutdown();
    });
}

/// WHY: A proxy must strip hop-by-hop headers and append X-Forwarded-For.
/// WHAT: httpbin echoes the headers it received; the hop-by-hop `X-Hop` named in
/// `Connection` is absent, and `X-Forwarded-For` is present.
#[test]
#[ignore = "requires Docker daemon"]
fn test_hop_by_hop_stripped_and_xff_added() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    RT.block_on(async {
        let backend = ContainerHandle::start_async(httpbin_config())
            .await
            .expect("start httpbin");
        let url = backend_url(&backend, HTTPBIN_PORT);

        let proxy = start_proxy(vec![
            ServiceConfig::new("app", "app.local").backend(&url)
        ]).await;
        let addr = proxy.local_addr();

        // `Connection: close, x-hop` marks `x-hop` as hop-by-hop; it must not
        // reach the backend. httpbin's /headers echoes received headers as JSON.
        let resp = send_request(
            addr,
            "GET",
            "/headers",
            "app.local",
            &[("X-Hop", "secret"), ("Connection", "close, x-hop")],
        );
        assert_eq!(resp.status, 200);
        let body_lower = resp.body.to_lowercase();
        assert!(
            !body_lower.contains("x-hop"),
            "hop-by-hop header must be stripped, body: {}",
            resp.body
        );
        assert!(
            body_lower.contains("x-forwarded-for"),
            "X-Forwarded-For must be added, body: {}",
            resp.body
        );

        proxy.shutdown();
        backend.shutdown_async().await.ok();
    });
}

/// WHY: `tcp://` backends are proxied byte-for-byte with no HTTP semantics.
/// WHAT: A raw TcpPassthrough in front of an http-echo backend round-trips a raw
/// HTTP request and returns the backend's bytes.
#[test]
#[ignore = "requires Docker daemon"]
fn test_tcp_passthrough_roundtrips_raw_bytes() {
    if !docker_available() {
        return;
    }
    RT.block_on(async {
        let backend = ContainerHandle::start_async(echo_config("tcp-backend"))
            .await
            .expect("start backend");
        let port = backend.host_port(ECHO_PORT).expect("mapped");
        let authority = format!("127.0.0.1:{port}");

        let passthrough =
            TcpPassthrough::start("127.0.0.1:0", &authority).expect("start passthrough");
        let addr = passthrough.local_addr();

        // Speak raw HTTP through the passthrough; the bytes reach http-echo and
        // its response bytes come straight back.
        let resp = send_request(addr, "GET", "/", "whatever", &[]);
        assert!(
            resp.body.contains("tcp-backend"),
            "raw passthrough must return backend bytes, got {:?}",
            resp.body
        );

        passthrough.shutdown();
        backend.shutdown_async().await.ok();
    });
}

/// WHY: Health probes must eject an unhealthy backend and readmit it on
/// recovery — traffic follows.
/// WHAT: Two backends behind one service; stopping one routes all traffic to the
/// survivor, and restarting it (same host port) returns it to rotation.
#[test]
#[ignore = "requires Docker daemon"]
fn test_health_ejects_and_readmits_backend() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    RT.block_on(async {
        // Fixed host port so the restarted container is reachable at the same URL.
        let fixed_port = 53991u16;
        let survivor = ContainerHandle::start_async(echo_config("survivor"))
            .await
            .expect("start survivor");
        let flaky = ContainerHandle::start_async(echo_config_fixed("flaky", fixed_port))
            .await
            .expect("start flaky");
        let flaky_url = format!("http://127.0.0.1:{fixed_port}");

        let hc = HealthCheckConfig {
            path: "/".into(),
            interval: Duration::from_millis(300),
            timeout: Duration::from_secs(1),
            healthy_threshold: 1,
            unhealthy_threshold: 2,
        };
        let proxy = start_proxy(vec![ServiceConfig::new("app", "app.local")
            .backend_target(BackendTarget::new(backend_url(&survivor, ECHO_PORT)))
            .backend_target(BackendTarget::new(&flaky_url))
            .health_check_config(hc)]).await;
        let addr = proxy.local_addr();

        // Stop the flaky backend; after the probes eject it all traffic is the
        // survivor's.
        flaky.shutdown_async().await.ok();
        wait_until_only(addr, "survivor", "flaky").await;

        // Restart flaky on the same port; it must return to rotation.
        let flaky2 = ContainerHandle::start_async(echo_config_fixed("flaky", fixed_port))
            .await
            .expect("restart flaky");
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut saw_flaky_again = false;
        while Instant::now() < deadline {
            let resp = send_request(addr, "GET", "/", "app.local", &[]);
            if resp.body.contains("flaky") {
                saw_flaky_again = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        assert!(saw_flaky_again, "recovered backend must return to rotation");

        proxy.shutdown();
        survivor.shutdown_async().await.ok();
        flaky2.shutdown_async().await.ok();
    });
}

/// Poll until every response identifies `expected` and never `forbidden`.
async fn wait_until_only(addr: std::net::SocketAddr, expected: &str, forbidden: &str) {
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        let mut all_expected = true;
        for _ in 0..5 {
            let resp = send_request(addr, "GET", "/", "app.local", &[]);
            if resp.body.contains(forbidden) || !resp.body.contains(expected) {
                all_expected = false;
                break;
            }
        }
        if all_expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    panic!("traffic never converged onto {expected} after {forbidden} was stopped");
}
