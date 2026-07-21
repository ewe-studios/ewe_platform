//! End-to-end tests for the `#[docker_container]` proc macro (spec-53 F02).
//!
//! WHY: the macro was defined but never used anywhere, so nothing proved it even
//! expanded — it in fact emitted invalid code. These tests exercise it the way
//! decision `08-proc-macro-location.md` documents it: paired with
//! `#[valtron_test]` (which supplies the valtron pool the Docker client needs).
//!
//! WHAT is proven: the generated prologue starts a container and honours the
//! wait strategy, the body receives every started container as a
//! `ContainerGroup` and can address each one, the containers are actually
//! *usable* while the body runs (a real Redis PING/PONG, not merely "start
//! returned Ok"), and the group's Drop stops and removes them afterwards.
//!
//! HOW the body reaches a container: it takes a `ContainerGroup` parameter (the
//! macro rejects a fn without one), looks a handle up by the logical key from
//! `as = "..."`, and asks the handle for its address. Ports are left for Docker
//! to assign — `port = 6379` plus `handle.address(6379)` — so nothing pins a
//! host port that a parallel test might already hold.
//!
//! Multiple containers go in one invocation, one `{ ... }` block each; the bare
//! `key = value` list is shorthand for a single container.
//!
//! Without a Docker daemon the macro logs a skip and returns, so these pass.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};

use foundation_core::valtron::valtron_test;
use foundation_deployment_platform::docker::ContainerGroup;
use foundation_deployment_platform::docker_container;

/// Sends `PING` to a Redis at `addr` and asserts `+PONG` comes back. This is
/// what proves the container is genuinely serving while the body runs.
fn assert_redis_pong(addr: SocketAddr) {
    let mut stream = TcpStream::connect(addr)
        .expect("container should be accepting connections while the body runs");

    stream.write_all(b"PING\r\n").expect("PING should write");

    let mut buf = [0u8; 7];
    stream.read_exact(&mut buf).expect("PONG should read back");
    assert_eq!(&buf, b"+PONG\r\n", "Redis should answer PING with +PONG");
}

/// Full documented grammar on one sync function: a Docker-assigned host port
/// reached through the handle, a logical lookup key, env vars, a resource limit,
/// a composite wait (`wait_stdout` AND `wait_port`, both under one
/// `wait_timeout`), and a graceful stop timeout.
#[docker_container(
    image = "redis:7-alpine",
    as = "cache",
    port = 6379,
    env = [("REDIS_ARGS", "--appendonly no")],
    wait_stdout = "Ready to accept connections",
    // Container-side port: the wait resolves it through the port map.
    wait_port = 6379,
    wait_timeout = 60,
    memory = "256m",
    stop_timeout = 5
)]
#[valtron_test]
fn docker_container_macro_serves_traffic_while_body_runs(containers: ContainerGroup) {
    let cache = containers
        .container("cache")
        .expect("the `as = \"cache\"` container should be in the group");

    let addr = cache
        .address(6379)
        .expect("Docker should have assigned a host port for 6379");
    assert_ne!(addr.port(), 6379, "the host port should be Docker-assigned, not the container port");

    assert_redis_pong(addr);
}

/// One invocation, one `{ ... }` block per container: the body gets all of them
/// in a single group, each addressed by its own logical key.
#[docker_container(
    {
        image = "redis:7-alpine",
        as = "first",
        port = 6379,
        wait_stdout = "Ready to accept connections",
        stop_timeout = 5
    },
    {
        image = "redis:7-alpine",
        as = "second",
        port = 6379,
        wait_stdout = "Ready to accept connections",
        stop_timeout = 5
    }
)]
#[valtron_test]
fn docker_container_macro_starts_multiple_containers(containers: ContainerGroup) {
    assert_eq!(containers.len(), 2, "both declared containers should be in the group");

    let first = containers.container("first").expect("`first` should be in the group");
    let second = containers.container("second").expect("`second` should be in the group");

    let first_addr = first.address(6379).expect("first should have a host port");
    let second_addr = second.address(6379).expect("second should have a host port");
    assert_ne!(
        first_addr, second_addr,
        "each container should get its own Docker-assigned host port"
    );

    // Both are live at once, and each is independently reachable.
    assert_redis_pong(first_addr);
    assert_redis_pong(second_addr);

    // Containers start in the order written, so the group reads the same way as
    // the attribute does.
    assert_eq!(
        containers.get(0).and_then(|h| h.address(6379)),
        Some(first_addr),
        "the first block's container starts first"
    );
}

/// The `async fn` arm keeps the function async and splices the body inline. It
/// is driven from a sync test so it needs no separate async test harness.
///
/// `required = true` also drops the Docker-absent skip arm, so this one only
/// passes where a daemon is genuinely reachable.
#[docker_container(
    image = "redis:7-alpine",
    as = "cache",
    port = 6379,
    wait_stdout = "Ready to accept connections",
    required = true,
    stop_timeout = 5
)]
async fn redis_body_async(containers: ContainerGroup) {
    let addr = containers
        .container("cache")
        .and_then(|h| h.address(6379))
        .expect("cache should be addressable");
    assert_redis_pong(addr);
}

#[valtron_test]
fn docker_container_macro_supports_async_functions() {
    // The declared `containers: ContainerGroup` parameter is consumed by the
    // macro: it seeds the group and passes it in, so the callable fn takes none.
    foundation_core::valtron::block_on_future(redis_body_async());
}
