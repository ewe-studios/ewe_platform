//! End-to-end tests for the `#[docker_container]` proc macro (spec-53 F02).
//!
//! WHY: the macro was defined but never used anywhere, so nothing proved it even
//! expanded — it in fact emitted invalid code. These tests exercise it the way
//! decision `08-proc-macro-location.md` documents it: stacked with
//! `#[valtron_test]` (which supplies the valtron pool the Docker client needs).
//!
//! WHAT is proven: the generated prologue starts a container and honours the
//! wait strategy, the body runs while that container is live and *usable* (a
//! real Redis PING/PONG over the mapped port, not merely "start returned Ok"),
//! and the generated epilogue drops the handle so the container is stopped and
//! removed afterwards.
//!
//! HOW the body reaches the container: `port_mapped = (container, host)` pins a
//! known host port. The macro deliberately keeps the handle in a hidden local
//! (per the decision doc's generated shape), so an explicit mapping is how a
//! body addresses its container.
//!
//! Without a Docker daemon the macro logs a skip and returns, so these pass.

use std::io::{Read, Write};
use std::net::TcpStream;

use foundation_core::valtron::valtron_test;
use foundation_deployment_platform::docker_container;

/// Host ports pinned for these tests — high, and unlikely to collide. Every
/// test gets its own pair: the harness runs tests in parallel, and two live
/// containers asking for the same host port make Docker fail the second start.
const REDIS_HOST_PORT: u16 = 16399;
const REDIS_STACKED_A_PORT: u16 = 16398;
const REDIS_STACKED_B_PORT: u16 = 16396;
const REDIS_ASYNC_PORT: u16 = 16397;

/// Sends `PING` to a Redis on `port` and asserts `+PONG` comes back. This is
/// what proves the container is genuinely serving while the body runs.
fn assert_redis_pong(port: u16) {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .expect("container should be accepting connections while the body runs");

    stream.write_all(b"PING\r\n").expect("PING should write");

    let mut buf = [0u8; 7];
    stream.read_exact(&mut buf).expect("PONG should read back");
    assert_eq!(&buf, b"+PONG\r\n", "Redis should answer PING with +PONG");
}

/// Full documented grammar on one sync function: an explicit port mapping, env
/// vars, a resource limit, a composite wait (`wait_stdout` AND `wait_port`, both
/// under one `wait_timeout`), and a graceful stop timeout.
#[docker_container(
    image = "redis:7-alpine",
    port_mapped = (6379, 16399),
    env = [("REDIS_ARGS", "--appendonly no")],
    wait_stdout = "Ready to accept connections",
    // Container-side port: the wait resolves it through the port map.
    wait_port = 6379,
    wait_timeout = 60,
    memory = "256m",
    stop_timeout = 5
)]
#[valtron_test]
fn docker_container_macro_serves_traffic_while_body_runs() {
    assert_redis_pong(REDIS_HOST_PORT);
}

/// Stacked attributes start one container per attribute, and both are live for
/// the body — the multi-container composition the decision doc describes.
#[docker_container(
    image = "redis:7-alpine",
    port_mapped = (6379, 16398),
    wait_stdout = "Ready to accept connections",
    stop_timeout = 5
)]
#[docker_container(
    image = "redis:7-alpine",
    port_mapped = (6379, 16396),
    wait_stdout = "Ready to accept connections",
    stop_timeout = 5
)]
#[valtron_test]
fn docker_container_macro_stacks_multiple_containers() {
    assert_redis_pong(REDIS_STACKED_A_PORT);
    assert_redis_pong(REDIS_STACKED_B_PORT);
}

/// The `async fn` arm keeps the function async and awaits the body inline. It is
/// driven from a sync test so it needs no separate async test harness.
///
/// `required = true` also drops the Docker-absent skip arm, so this one only
/// passes where a daemon is genuinely reachable.
#[docker_container(
    image = "redis:7-alpine",
    port_mapped = (6379, 16397),
    wait_stdout = "Ready to accept connections",
    required = true,
    stop_timeout = 5
)]
async fn redis_body_async() {
    assert_redis_pong(REDIS_ASYNC_PORT);
}

#[valtron_test]
fn docker_container_macro_supports_async_functions() {
    foundation_core::valtron::block_on_future(redis_body_async());
}
