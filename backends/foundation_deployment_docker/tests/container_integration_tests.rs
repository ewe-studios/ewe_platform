//! Integration tests for container lifecycle methods.
//!
//! These tests exercise the Docker container lifecycle API against a real
//! dockerd: kill, pause/unpause, wait, resize, changes, list, and prune.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test container_integration_tests -- --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

/// One fresh client per test — no shared state, no pool interference.
fn client() -> DockerClient {
    DockerClient::connect_unix("/var/run/docker.sock")
}

// ── container_list_with_filters ─────────────────────────────────────────

#[valtron_test]
async fn container_list_with_filters() {
    let c = client();
    let name = "ewe-it-ct-list";
    let id = c
        .create_container(
            &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
            Some(name),
        )
        .await
        .expect("create");

    let list = c
        .container_list(true, None, false, None)
        .await
        .expect("list");
    let arr = list.as_array().expect("should be JSON array");
    let found = arr.iter().any(|ctr| {
        ctr.get("Names")
            .and_then(|n| n.as_array())
            .and_then(|a| a.first())
            .and_then(|n| n.as_str())
            .map(|n| n == &format!("/{name}"))
            .unwrap_or(false)
    });
    assert!(found, "container list should contain /{name}");

    let _ = c.remove_container(&id.id, true).await;
}

// ── container_kill ──────────────────────────────────────────────────────

#[valtron_test]
async fn container_kill() {
    let c = client();
    let id = c
        .create_container(
            &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
            Some("ewe-it-ct-kill"),
        )
        .await
        .expect("create");
    c.start_container(&id.id).await.expect("start");

    c.kill_container(&id.id, None).await.expect("kill");

    let inspect = c.inspect_container(&id.id).await.expect("inspect");
    let running = inspect
        .state
        .as_ref()
        .and_then(|s| s.running)
        .unwrap_or(true);
    assert!(!running, "container should not be running after kill");

    let _ = c.remove_container(&id.id, true).await;
}

// ── container_pause_then_unpause ─────────────────────────────────────────

#[valtron_test]
async fn container_pause_then_unpause() {
    let c = client();
    let id = c
        .create_container(
            &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
            Some("ewe-it-ct-pause"),
        )
        .await
        .expect("create");
    c.start_container(&id.id).await.expect("start");

    c.pause_container(&id.id).await.expect("pause");
    let inspect = c.inspect_container(&id.id).await.expect("inspect");
    let paused = inspect
        .state
        .as_ref()
        .and_then(|s| s.paused)
        .unwrap_or(false);
    assert!(paused, "container should be paused");

    c.unpause_container(&id.id).await.expect("unpause");
    let inspect = c.inspect_container(&id.id).await.expect("inspect");
    let running = inspect
        .state
        .as_ref()
        .and_then(|s| s.running)
        .unwrap_or(false);
    assert!(running, "container should be running after unpause");

    c.stop_container(&id.id, Some(5)).await.expect("stop");
    let _ = c.remove_container(&id.id, true).await;
}

// ── container_prune ─────────────────────────────────────────────────────

#[valtron_test]
async fn container_prune() {
    let c = client();
    let id = c
        .create_container(
            &serde_json::json!({"Image": "alpine:latest", "Cmd": ["true"]}),
            Some("ewe-it-ct-prune"),
        )
        .await
        .expect("create");
    // Start the container — "true" exits immediately, leaving it in exited/stopped state
    let _ = c.start_container(&id.id).await;
    // Give it a moment to exit
    std::thread::sleep(std::time::Duration::from_secs(2));

    c.container_prune(None).await.expect("prune");
}

// ── container_resize_tty ────────────────────────────────────────────────

#[valtron_test]
async fn container_resize_tty() {
    let c = client();
    let id = c
        .create_container(
            &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
            Some("ewe-it-ct-resize"),
        )
        .await
        .expect("create");
    c.start_container(&id.id).await.expect("start");

    c.resize_container(&id.id, Some(40), Some(120))
        .await
        .expect("resize");

    c.stop_container(&id.id, Some(5)).await.expect("stop");
    let _ = c.remove_container(&id.id, true).await;
}

// ── container_wait ──────────────────────────────────────────────────────

#[valtron_test]
async fn container_wait() {
    let c = client();
    let id = c
        .create_container(
            &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "5"]}),
            Some("ewe-it-ct-wait"),
        )
        .await
        .expect("create");
    c.start_container(&id.id).await.expect("start");

    let resp = c
        .wait_container(&id.id, Some("not-running"))
        .await
        .expect("wait");
    assert_eq!(resp.status_code, 0, "exit code should be 0");

    let _ = c.remove_container(&id.id, true).await;
}

// ── container_changes ───────────────────────────────────────────────────

#[valtron_test]
async fn container_changes() {
    // Clean up from prior aborted run
    let _ = std::process::Command::new("docker")
        .args(["rm", "-f", "ewe-it-ct-changes"])
        .output();
    let c = client();
    let id = c
        .create_container(
            &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
            Some("ewe-it-ct-changes"),
        )
        .await
        .expect("create");
    c.start_container(&id.id).await.expect("start");

    let _changes = c.container_changes(&id.id).await.expect("changes");
    // Docker returns a JSON array (or null for a fresh unchanged container)

    c.stop_container(&id.id, Some(5)).await.expect("stop");
    let _ = c.remove_container(&id.id, true).await;
}
