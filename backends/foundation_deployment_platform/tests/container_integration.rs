//! Integration tests for ContainerHandle — requires Docker.
//! Run with: cargo test -p foundation_deployment_platform --test container_integration

use std::sync::LazyLock;
use foundation_deployment_platform::docker::{
    ContainerConfig, ContainerGroup, ContainerHandle, DockerError,
};

/// Single-threaded tokio runtime for integration tests.
static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime")
});

/// Helper: check if Docker is available.
fn docker_available() -> bool {
    std::path::Path::new("/var/run/docker.sock").exists()
}

#[test]
#[ignore = "requires Docker daemon running"]
fn test_start_redis_container() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }

    RT.block_on(async {
        let config = ContainerConfig::new("redis:7-alpine")
            .port(6379)
            .stop_timeout_secs(5);

        let handle = ContainerHandle::start_async(config)
            .await
            .expect("failed to start Redis container");

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
        handle.shutdown_async().await.expect("failed to shutdown container");
        assert!(!handle.is_running(), "container should not be running after shutdown");
    });
}

#[test]
#[ignore = "requires Docker daemon running"]
fn test_container_auto_cleanup_on_drop() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }

    RT.block_on(async {
        {
            let config = ContainerConfig::new("redis:7-alpine")
                .port(6379)
                .stop_timeout_secs(2);

            let handle = ContainerHandle::start_async(config)
                .await
                .expect("failed to start Redis container");
            drop(handle); // Drop should stop + remove
        }

        // After Drop, the container should be gone
        let check_handle = ContainerHandle::start_async(
            ContainerConfig::new("redis:7-alpine")
                .port(6379)
                .stop_timeout_secs(2),
        ).await;
        assert!(
            check_handle.is_ok(),
            "container was cleaned up — a new container with the same port should start"
        );
    });
}

#[test]
fn test_docker_unavailable_returns_connection_error() {
    if docker_available() {
        eprintln!("SKIP: Docker is available — this test is for the absent case");
        return;
    }

    RT.block_on(async {
        let result = ContainerHandle::start_async(
            ContainerConfig::new("redis:7-alpine").port(6379),
        ).await;

        match result {
            Err(e) => {
                assert!(
                    e.current_context().is_connection_error(),
                    "should be connection error when Docker is absent"
                );
            }
            Ok(_) => panic!("should not succeed when Docker is absent"),
        }
    });
}
