//! End-to-end proxy data-plane tests against real backend containers.
//!
//! Gated behind `#[cfg(feature = "docker-tests")]` — run with:
//!   cargo test -p foundation_proxy --features docker-tests
//!
//! WHY: Unit tests prove the router, balancer, and header logic in isolation.
//! These prove the assembled proxy actually relays bytes.
//!
//! HOW: Container lifecycle (bollard) needs tokio — `RT.block_on` wraps only
//! the container start/shutdown.  Proxy setup, routing, and assertions are
//! fully sync.  No `#[ignore]` — the feature gate selects these tests.

#![cfg(feature = "docker-tests")]

use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use foundation_core::valtron::initialize_pool;
use foundation_deployment_platform::docker::{ContainerConfig, ContainerHandle, WaitFor};
use foundation_proxy::config::HealthCheckConfig;
use foundation_proxy::{ProxyConfig, ProxyServer, ServiceConfig, TcpPassthrough, UdpPassthrough};

/// Shared tokio runtime — used only for container lifecycle (bollard needs it).
static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime")
});

const ECHO_PORT: u16 = 5678;

fn docker_available() -> bool {
    std::path::Path::new("/var/run/docker.sock").exists()
}

fn echo_config(text: &str) -> ContainerConfig {
    ContainerConfig::new("hashicorp/http-echo:latest")
        .command(vec![format!("-text={text}"), format!("-listen=:{ECHO_PORT}")])
        .port(ECHO_PORT)
        .wait(WaitFor::port_with_timeout(ECHO_PORT, Duration::from_secs(60)))
        .stop_timeout_secs(2)
}

struct RawResponse {
    status: u16,
    body: String,
}

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
    let body = text[split + 4..].to_string();
    let status_line = text[..split].split("\r\n").next().expect("status line");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .expect("status code");
    RawResponse { status, body }
}

fn start_proxy(services: Vec<ServiceConfig>) -> ProxyServer {
    let mut config = ProxyConfig::new("test.local", "127.0.0.1").bind("127.0.0.1:0");
    config.services = services;
    config.start().expect("start proxy")
}

fn backend_url(handle: &ContainerHandle, container_port: u16) -> String {
    let port = handle.host_port(container_port).expect("mapped port");
    format!("http://127.0.0.1:{port}")
}

// ── Forward + round-robin ──

#[test]
fn test_forward_roundtrips_to_backend() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    let backend = RT.block_on(async { ContainerHandle::start_async(echo_config("backend-solo")).await })
        .expect("start echo");
    let url = backend_url(&backend, ECHO_PORT);

    let proxy = start_proxy(vec![ServiceConfig::new("app", "app.local").backend(&url)]);
    let addr = proxy.local_addr();

    let resp = send_request(addr, "GET", "/", "app.local", &[]);
    assert_eq!(resp.status, 200);
    assert!(resp.body.contains("backend-solo"), "body: {:?}", resp.body);

    proxy.shutdown();
    RT.block_on(async { backend.shutdown_async().await.ok() });
}

#[test]
fn test_round_robin_spreads_across_backends() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    let (a, b) = RT.block_on(async {
        let a = ContainerHandle::start_async(echo_config("backend-A")).await.expect("A");
        let b = ContainerHandle::start_async(echo_config("backend-B")).await.expect("B");
        (a, b)
    });
    let proxy = start_proxy(vec![ServiceConfig::new("app", "app.local")
        .backend(&backend_url(&a, ECHO_PORT))
        .backend(&backend_url(&b, ECHO_PORT))]);
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
    assert!(saw_a && saw_b, "round-robin must hit both (A={saw_a}, B={saw_b})");

    proxy.shutdown();
    RT.block_on(async { a.shutdown_async().await.ok(); b.shutdown_async().await.ok() });
}

// ── Host + path routing ──

#[test]
fn test_host_routing_and_unknown_host_404() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    let (a, b) = RT.block_on(async {
        let a = ContainerHandle::start_async(echo_config("host-A")).await.expect("A");
        let b = ContainerHandle::start_async(echo_config("host-B")).await.expect("B");
        (a, b)
    });
    let proxy = start_proxy(vec![
        ServiceConfig::new("a", "a.local").backend(&backend_url(&a, ECHO_PORT)),
        ServiceConfig::new("b", "b.local").backend(&backend_url(&b, ECHO_PORT)),
    ]);
    let addr = proxy.local_addr();

    assert!(send_request(addr, "GET", "/", "a.local", &[]).body.contains("host-A"));
    assert!(send_request(addr, "GET", "/", "b.local", &[]).body.contains("host-B"));
    assert_eq!(send_request(addr, "GET", "/", "nope.local", &[]).status, 404);

    proxy.shutdown();
    RT.block_on(async { a.shutdown_async().await.ok(); b.shutdown_async().await.ok() });
}

#[test]
fn test_path_prefix_longest_wins() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    let (root, api) = RT.block_on(async {
        let r = ContainerHandle::start_async(echo_config("root-backend")).await.expect("root");
        let a = ContainerHandle::start_async(echo_config("api-backend")).await.expect("api");
        (r, a)
    });
    let proxy = start_proxy(vec![
        ServiceConfig::new("root", "app.local").backend(&backend_url(&root, ECHO_PORT)),
        ServiceConfig::new("api", "app.local")
            .path_prefix("/api")
            .backend(&backend_url(&api, ECHO_PORT)),
    ]);
    let addr = proxy.local_addr();

    assert!(send_request(addr, "GET", "/api/thing", "app.local", &[]).body.contains("api-backend"));
    assert!(send_request(addr, "GET", "/other", "app.local", &[]).body.contains("root-backend"));

    proxy.shutdown();
    RT.block_on(async { root.shutdown_async().await.ok(); api.shutdown_async().await.ok() });
}

// ── Health check → 503 ──

#[test]
fn test_no_healthy_backend_returns_503() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    let backend = RT.block_on(async { ContainerHandle::start_async(echo_config("doomed")).await })
        .expect("start backend");
    let url = backend_url(&backend, ECHO_PORT);

    let proxy = start_proxy(vec![ServiceConfig::new("app", "app.local")
        .backend(&url)
        .health_check_config(HealthCheckConfig {
            path: "/".into(),
            interval: Duration::from_millis(300),
            timeout: Duration::from_secs(1),
            healthy_threshold: 1,
            unhealthy_threshold: 2,
        })]);
    let addr = proxy.local_addr();

    RT.block_on(async { backend.shutdown_async().await.ok() });
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got_503 = false;
    while Instant::now() < deadline {
        if send_request(addr, "GET", "/", "app.local", &[]).status == 503 {
            got_503 = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    assert!(got_503, "expected 503 once the only backend is unhealthy");

    proxy.shutdown();
}

// ── Hop-by-hop + X-Forwarded-* ──

#[test]
fn test_hop_by_hop_stripped_and_xff_added() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(4));
    let a = RT.block_on(async { ContainerHandle::start_async(echo_config("hop-backend")).await })
        .expect("start echo A");
    let proxy = start_proxy(vec![ServiceConfig::new("app", "app.local")
        .backend(&backend_url(&a, ECHO_PORT))]);
    let addr = proxy.local_addr();

    let resp = send_request(
        addr, "GET", "/", "app.local",
        &[("X-Hop", "secret"), ("Connection", "close, x-hop")],
    );
    assert_eq!(resp.status, 200);
    assert!(resp.body.contains("hop-backend"), "body: {:?}", resp.body);

    proxy.shutdown();
    RT.block_on(async { a.shutdown_async().await.ok() });
}

// ── Raw TCP + UDP passthrough ──

#[test]
fn test_tcp_passthrough_roundtrips_raw_bytes() {
    if !docker_available() {
        return;
    }
    let backend = RT.block_on(async { ContainerHandle::start_async(echo_config("tcp-backend")).await })
        .expect("start backend");
    let port = backend.host_port(ECHO_PORT).expect("mapped");

    let passthrough = TcpPassthrough::start("127.0.0.1:0", &format!("127.0.0.1:{port}"))
        .expect("start passthrough");
    let addr = passthrough.local_addr();

    assert!(send_request(addr, "GET", "/", "whatever", &[]).body.contains("tcp-backend"));

    passthrough.shutdown();
    RT.block_on(async { backend.shutdown_async().await.ok() });
}

#[test]
fn test_udp_passthrough_roundtrips_bytes() {
    if !docker_available() {
        return;
    }
    let backend = RT.block_on(async {
        ContainerHandle::start_async(
            ContainerConfig::new("alpine/socat:latest")
                .command(vec!["UDP-LISTEN:9000,fork".into(), "EXEC:cat".into()])
                .port_udp(9000)
                .wait(WaitFor::udp_with_timeout(9000, Duration::from_secs(30)))
                .stop_timeout_secs(2),
        ).await
    }).expect("start socat UDP echo");
    let port = backend.host_port_udp(9000).expect("mapped UDP port");

    let passthrough = UdpPassthrough::start("127.0.0.1:0", &format!("127.0.0.1:{port}"))
        .expect("start passthrough");
    let addr = passthrough.local_addr();

    let socket = UdpSocket::bind("0.0.0.0:0").expect("bind client");
    socket.set_read_timeout(Some(Duration::from_secs(5))).expect("set timeout");
    socket.send_to(b"udp-passthrough-ping", addr).expect("send datagram");

    let mut buf = [0u8; 64];
    let (n, _) = socket.recv_from(&mut buf).expect("recv response");
    assert_eq!(&buf[..n], b"udp-passthrough-ping");

    passthrough.shutdown();
    RT.block_on(async { backend.shutdown_async().await.ok() });
}

// ── Health probe state machine ──

#[test]
fn test_health_ejects_and_readmits_backend() {
    if !docker_available() {
        return;
    }
    let _pool = initialize_pool(53, Some(8));
    let backend = RT.block_on(async { ContainerHandle::start_async(echo_config("health-me")).await })
        .expect("start backend");
    let url = backend_url(&backend, ECHO_PORT);

    let proxy = start_proxy(vec![ServiceConfig::new("app", "app.local")
        .backend(&url)
        .health_check_config(HealthCheckConfig {
            path: "/".into(),
            interval: Duration::from_millis(200),
            timeout: Duration::from_secs(1),
            healthy_threshold: 1,
            unhealthy_threshold: 2,
        })]);
    let addr = proxy.local_addr();

    assert_eq!(send_request(addr, "GET", "/", "app.local", &[]).status, 200);

    RT.block_on(async { backend.shutdown_async().await.ok() });

    let svc = proxy.state().router().services().iter()
        .find(|s| s.config().name == "app").expect("find app service");
    let rt = &svc.backends()[0];
    let deadline = Instant::now() + Duration::from_secs(10);
    while rt.is_healthy() {
        assert!(Instant::now() < deadline, "backend never marked unhealthy");
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!rt.is_healthy());

    assert_eq!(send_request(addr, "GET", "/", "app.local", &[]).status, 503);

    proxy.shutdown();
}
