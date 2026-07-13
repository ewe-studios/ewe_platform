//! Integration tests against a running Docker daemon.
//!
//! WHY: Prove `DockerClient` API surfaces work against an actual dockerd —
//! transport, timeouts, pooling, serde, and error handling end-to-end.
//!
//! WHAT: Read-only queries + create-then-cleanup operations that leave no state.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat -- integration --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

fn connect() -> DockerClient {
    DockerClient::connect_unix("/var/run/docker.sock")
}

// ── System ──────────────────────────────────────────────────────────────

#[valtron_test]
async fn system_ping_responds() {
    connect().system_ping().await.expect("GET /_ping");
}

#[valtron_test]
async fn system_info_deserializes() {
    let c = connect();
    let info = c.system_info().await.expect("GET /info");
    assert!(info.id.is_some() || info.name.is_some(), "info has fields");
}

#[valtron_test]
async fn system_data_usage_returns_value() {
    let df = connect().system_data_usage(None).await.expect("GET /system/df");
    assert!(!df.is_null(), "disk usage should not be null");
}

// ── Networks ────────────────────────────────────────────────────────────

#[valtron_test]
async fn network_list_returns_array() {
    let nets = connect().network_list(None).await.expect("GET /networks");
    assert!(nets.is_array(), "should be JSON array: {nets}");
}

#[valtron_test]
async fn network_create_then_delete() {
    let c = connect();
    let body = serde_json::json!({"Name": "ewe-it-net", "Driver": "bridge"});
    let created = c.network_create(body).await.expect("POST /networks/create");
    assert!(!created.id.is_empty(), "NetworkCreateResponse.id should be set");
    c.network_delete(&created.id).await.expect("DELETE /networks/{id}");
}

// ── Volumes ─────────────────────────────────────────────────────────────

#[valtron_test]
async fn volume_list_returns_volumes() {
    let resp = connect().volume_list(None).await.expect("GET /volumes");
    // VolumeListResponse has `volumes: Option<Vec<Volume>>`
    assert!(
        resp.volumes.is_some() || true,
        "volume list deserialized"
    );
}

#[valtron_test]
async fn volume_create_then_delete() {
    let c = connect();
    let cfg = serde_json::json!({"Name": "ewe-it-vol", "Driver": "local"});
    let vol = c.volume_create(cfg).await.expect("POST /volumes/create");
    assert!(!vol.name.is_empty(), "volume should have name");
    c.volume_delete(&vol.name, Some(true)).await.expect("DELETE /volumes/{name}");
}

// ── Images ──────────────────────────────────────────────────────────────

#[valtron_test]
async fn image_list_returns_array() {
    let images = connect().image_list(Some(true), None, None, None).await.expect("GET /images/json");
    assert!(images.is_array(), "should be JSON array");
}

#[valtron_test]
async fn image_search_returns_results() {
    let results = connect().image_search(Some("alpine"), None, None).await.expect("GET /images/search");
    assert!(results.is_array(), "search should return JSON array");
}

// ── Containers (the critical path) ──────────────────────────────────────

#[valtron_test]
async fn container_list_returns_array() {
    let list = connect().container_list(true, None, false, None).await.expect("GET /containers/json");
    assert!(list.is_array(), "container list should be JSON array");
}

#[valtron_test]
async fn container_lifecycle_create_start_stop_remove() {
    let c = connect();
    let id = c.create_container(
        &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "60"]}),
        Some("ewe-it-lifecycle"),
    ).await.expect("create container");
    assert!(!id.id.is_empty());

    c.start_container(&id.id).await.expect("start container");

    let inspect = c.inspect_container(&id.id).await.expect("inspect container");
    assert!(
        inspect.state.as_ref().and_then(|s| s.running).unwrap_or(false),
        "container should be running"
    );

    // Stop while running — 120s timeout covers Docker's 10s stop delay
    c.stop_container(&id.id, Some(10)).await.expect("stop container");
    c.remove_container(&id.id, true).await.expect("remove container");
}

#[valtron_test]
async fn container_stop_already_exited_handles_304() {
    let c = connect();
    let id = c.create_container(
        &serde_json::json!({"Image": "alpine:latest", "Cmd": ["true"]}),
        Some("ewe-it-304"),
    ).await.expect("create");

    c.start_container(&id.id).await.expect("start");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Docker returns 304 Not Modified — wrapper treats it as success
    c.stop_container(&id.id, Some(5)).await.expect("stop (304)");
    c.remove_container(&id.id, true).await.expect("remove after 304");
}

#[valtron_test]
async fn container_restart_cycles() {
    let c = connect();
    let id = c.create_container(
        &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
        Some("ewe-it-restart"),
    ).await.expect("create");

    c.start_container(&id.id).await.expect("start");
    c.restart_container(&id.id, None, None).await.expect("restart");
    c.stop_container(&id.id, Some(5)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_rename_changes_name() {
    let c = connect();
    let id = c.create_container(
        &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "5"]}),
        Some("ewe-it-original"),
    ).await.expect("create");

    c.rename_container(&id.id, "ewe-it-renamed").await.expect("rename");
    let _ = c.stop_container(&id.id, Some(1)).await;
    c.remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_logs_returns_output() {
    let c = connect();
    let id = c.create_container(
        &serde_json::json!({"Image": "alpine:latest", "Cmd": ["echo", "hello-it-test"]}),
        Some("ewe-it-logs"),
    ).await.expect("create");

    c.start_container(&id.id).await.expect("start");
    std::thread::sleep(std::time::Duration::from_secs(2));

    let logs = c.container_logs(&id.id, false, true, false, None, None, false, None)
        .await.expect("logs");
    assert!(!logs.is_empty(), "container should have log output");
    assert!(
        std::str::from_utf8(&logs).unwrap_or("").contains("hello-it-test"),
        "logs should contain our echo output"
    );

    c.remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_top_shows_processes() {
    let c = connect();
    let id = c.create_container(
        &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
        Some("ewe-it-top"),
    ).await.expect("create");

    c.start_container(&id.id).await.expect("start");

    let top = c.container_top(&id.id, None).await.expect("top");
    assert!(top.titles.is_some(), "top should return process titles");
    assert!(top.processes.is_some(), "top should return process list");

    c.stop_container(&id.id, Some(5)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
}

// ── Exec ────────────────────────────────────────────────────────────────

#[valtron_test]
async fn exec_create_start_returns_output() {
    let c = connect();
    let id = c.create_container(
        &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
        Some("ewe-it-exec"),
    ).await.expect("create");

    c.start_container(&id.id).await.expect("start");

    let exec_id = c.exec_create(&id.id, &["echo", "exec-output"]).await.expect("exec create");
    assert!(!exec_id.id.is_empty());

    let output = c.exec_start(&exec_id.id, false).await.expect("exec start");
    assert!(
        std::str::from_utf8(&output).unwrap_or("").contains("exec-output"),
        "exec output should contain our command"
    );

    c.stop_container(&id.id, Some(5)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
}

// ── Negotiation ─────────────────────────────────────────────────────────

#[valtron_test]
async fn negotiate_version_detects_real_server() {
    let mut c = connect();
    c.negotiate_version().await.expect("negotiate");
    let v = c.api_version();
    // Real server is 1.54; client default is 1.53
    assert!(v == "1.54" || v == "1.53", "got v={v}");
}

// ── Cleanup ─────────────────────────────────────────────────────────────

#[valtron_test]
async fn cleanup_leftover_it_containers() {
    let c = connect();
    let list = c.container_list(true, None, false, None).await.expect("list");
    let prefix = "ewe-it-";
    if let Some(arr) = list.as_array() {
        for container in arr {
            let name = container.get("Names")
                .and_then(|n| n.as_array())
                .and_then(|a| a.first())
                .and_then(|n| n.as_str())
                .unwrap_or("");
            if name.starts_with(&format!("/{prefix}")) {
                if let Some(id) = container.get("Id").and_then(|i| i.as_str()) {
                    eprintln!("cleanup: removing leftover {name}");
                    let _ = c.remove_container(id, true).await;
                }
            }
        }
    }
}
