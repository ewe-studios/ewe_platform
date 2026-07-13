//! Integration tests against a real Docker daemon on the default Unix socket.
//!
//! WHY: Proves `DockerClient` talks to an actual dockerd — version, container
//! create → start → inspect → pause → unpause → stop → remove lifecycle — over
//! the Unix-socket transport, driven by valtron.
//!
//! WHAT: Each test is `#[ignore]` by default (needs a running Docker daemon).
//! Run with: `cargo test -p foundation_deployment_docker --features docker
//!   --profile uat -- real_docker --include-ignored`
//!
//! HOW: `DockerClient::connect_unix("/var/run/docker.sock")`.

#![cfg(all(unix, feature = "docker"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

#[valtron_test]
#[ignore = "needs running Docker daemon on default socket"]
async fn real_docker_version() {
    let client = DockerClient::connect_unix("/var/run/docker.sock");
    let version = client.version().await.expect("GET /version");
    assert!(version.api_version.is_some(), "ApiVersion should be present");
    assert!(version.os.is_some(), "Os should be present");
    assert!(version.arch.is_some(), "Arch should be present");
}

#[valtron_test]
#[ignore = "needs running Docker daemon on default socket"]
async fn real_docker_create_start_inspect_stop_remove() {
    let client = DockerClient::connect_unix("/var/run/docker.sock");

    let id = client
        .create_container(
            &serde_json::json!({
                "Image": "alpine:latest",
                "Cmd": ["sleep", "30"],
            }),
            Some("ewe-real-docker-test"),
        )
        .await
        .expect("create container");
    assert!(!id.id.is_empty(), "container id should be returned");

    client.start_container(&id.id).await.expect("start container");

    let inspect = client.inspect_container(&id.id).await.expect("inspect container");
    assert!(
        inspect.state.as_ref().and_then(|s| s.running).unwrap_or(false),
        "container should be running after start"
    );

    client.stop_container(&id.id, Some(10)).await.expect("stop container");
    client.remove_container(&id.id, true).await.expect("remove container");
}

#[valtron_test]
#[ignore = "needs running Docker daemon on default socket"]
async fn real_docker_pause_unpause() {
    let client = DockerClient::connect_unix("/var/run/docker.sock");

    let id = client
        .create_container(
            &serde_json::json!({
                "Image": "alpine:latest",
                "Cmd": ["sleep", "30"],
            }),
            Some("ewe-pause-test"),
        )
        .await
        .expect("create container");

    client.start_container(&id.id).await.expect("start container");
    client.pause_container(&id.id).await.expect("pause container");

    let inspect = client.inspect_container(&id.id).await.expect("inspect after pause");
    assert!(
        inspect.state.as_ref().and_then(|s| s.paused).unwrap_or(false),
        "container should be paused"
    );

    client.unpause_container(&id.id).await.expect("unpause container");

    let inspect = client.inspect_container(&id.id).await.expect("inspect after unpause");
    assert!(
        inspect.state.as_ref().and_then(|s| s.running).unwrap_or(false),
        "container should be running after unpause"
    );

    client.stop_container(&id.id, Some(5)).await.expect("stop container");
    client.remove_container(&id.id, true).await.expect("remove container");
}

#[valtron_test]
#[ignore = "needs running Docker daemon"]
async fn real_docker_lifecycle_fixed() {
    let client = DockerClient::connect_unix("/var/run/docker.sock");

    let id = client
        .create_container(&serde_json::json!({
            "Image": "alpine:latest",
            "Cmd": ["sleep", "60"],
        }), Some("ewe-fix-test"))
        .await.expect("create");
    eprintln!("created: {}", id.id);

    client.start_container(&id.id).await.expect("start");
    eprintln!("started");

    // Stop immediately while container is still running
    client.stop_container(&id.id, Some(5)).await.expect("stop");
    eprintln!("stopped OK");

    client.remove_container(&id.id, true).await.expect("remove");
    eprintln!("removed OK — ALL DONE");
}
