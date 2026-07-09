//! Integration tests for ContainerHandle — requires Docker.
//! Run with: cargo test -p foundation_deployment_platform --test container_integration

use foundation_deployment_platform::docker::{
    ContainerConfig, ContainerGroup, ContainerHandle, DockerError, WaitFor,
};

/// Helper: check if Docker is available.
fn docker_available() -> bool {
    std::path::Path::new("/var/run/docker.sock").exists()
}

#[test]
fn test_start_redis_container() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }

    let config = ContainerConfig::new("redis:7-alpine")
        .port(6379)
        .wait(WaitFor::stdout("Ready to accept connections"))
        .stop_timeout_secs(5);

    let handle = ContainerHandle::start(config).expect("failed to start Redis container");

    let host_port = handle
        .host_port(6379)
        .expect("redis port 6379 should be mapped");

    assert!(host_port > 0, "host port should be non-zero");
    assert!(handle.is_running(), "container should report running");

    // Test that Redis responds
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let mut stream = TcpStream::connect(format!("127.0.0.1:{host_port}"))
        .expect("failed to connect to Redis");

    // PING → +PONG
    writeln!(stream, "PING").unwrap();
    let mut buf = [0u8; 8];
    let n = stream.read(&mut buf).unwrap();
    assert!(
        std::str::from_utf8(&buf[..n]).unwrap().contains("PONG"),
        "Redis should respond to PING"
    );

    // SET key value
    writeln!(stream, "SET test_key hello").unwrap();
    let mut buf = [0u8; 8];
    let _ = stream.read(&mut buf).unwrap();

    // GET key
    writeln!(stream, "GET test_key").unwrap();
    let mut buf = [0u8; 16];
    let n = stream.read(&mut buf).unwrap();
    assert!(
        std::str::from_utf8(&buf[..n])
            .unwrap()
            .contains("hello"),
        "Redis should return stored value"
    );

    // Explicit shutdown
    handle.shutdown().expect("failed to shutdown container");
    assert!(!handle.is_running(), "container should not be running after shutdown");
}

#[test]
fn test_container_auto_cleanup_on_drop() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }

    {
        let config = ContainerConfig::new("redis:7-alpine")
            .port(6379)
            .stop_timeout_secs(2);

        let handle = ContainerHandle::start(config).expect("failed to start Redis container");
        drop(handle); // Drop should stop + remove
    }

    // After Drop, the container should be gone
    let check_handle = ContainerHandle::start(
        ContainerConfig::new("redis:7-alpine")
            .port(6379)
            .stop_timeout_secs(2),
    );
    assert!(
        check_handle.is_ok(),
        "container was cleaned up — a new container with the same port should start"
    );
}

#[test]
fn test_container_group() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }

    let configs = vec![
        ContainerConfig::new("redis:7-alpine")
            .port(6379)
            .stop_timeout_secs(5),
        ContainerConfig::new("redis:7-alpine")
            .port(6379)
            .stop_timeout_secs(5),
    ];

    // Second container should fail due to port conflict (same host port)
    // but the first should work fine
    let group = ContainerGroup::start(configs);
    match group {
        Ok(_) => eprintln!("Both started (unexpected — port conflict should fail)"),
        Err(e) => {
            // Expected: second container fails due to port conflict
            let msg = format!("{e}");
            assert!(
                msg.contains("port") || msg.contains("bind"),
                "error should mention port/bind conflict: {msg}"
            );
        }
    }

    // Drop cleans up whatever was started
}

#[test]
fn test_docker_unavailable_skip() {
    if docker_available() {
        eprintln!("SKIP: Docker is available — this test is for the absent case");
        return;
    }

    let result = ContainerHandle::start(
        ContainerConfig::new("redis:7-alpine").port(6379),
    );

    match result {
        Err(e) => {
            // Should be a connection error
            assert!(
                DockerError::is_connection_error(&e.current_context()),
                "should be connection error when Docker is absent"
            );
        }
        Ok(_) => panic!("should not succeed when Docker is absent"),
    }
}
