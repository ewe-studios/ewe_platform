//! Integration tests for Networks, Volumes, Exec, and System API methods.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test network_volume_exec_integration_tests -- --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

/// One fresh client per test — no shared state, no pool interference.
fn client() -> DockerClient {
    DockerClient::connect_unix("/var/run/docker.sock")
}

// ── Network tests ──────────────────────────────────────────────────────

#[valtron_test]
async fn network_inspect_default_bridge() {
    let c = client();
    // The "bridge" network always exists on any Docker daemon.
    let _net = c
        .network_inspect("bridge", None, None)
        .await
        .expect("inspect bridge");
}

#[valtron_test]
async fn network_connect_then_disconnect() {
    let c = client();

    // Create a sleep container to connect.
    let ctr = c
        .create_container(
            &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
            Some("ewe-it-nve-netconnect"),
        )
        .await
        .expect("create container");
    c.start_container(&ctr.id).await.expect("start container");

    // Create a bridge network.
    let net = c
        .network_create(serde_json::json!({"Name":"ewe-it-nv-conntest","Driver":"bridge"}))
        .await
        .expect("create network");

    // Connect the container to the network.
    c.network_connect(&net.id, &ctr.id)
        .await
        .expect("connect");

    // Disconnect the container from the network.
    c.network_disconnect(&net.id, &ctr.id, false)
        .await
        .expect("disconnect");

    // Cleanup.
    c.stop_container(&ctr.id, Some(5)).await.expect("stop");
    c.remove_container(&ctr.id, true).await.expect("remove");
    c.network_delete(&net.id).await.expect("delete network");
}

#[valtron_test]
async fn network_prune() {
    let c = client();
    c.network_prune(None).await.expect("prune networks");
}

// ── Volume tests ───────────────────────────────────────────────────────

#[valtron_test]
async fn volume_inspect_created() {
    let c = client();

    let cfg = serde_json::json!({"Name":"ewe-it-nv-inspect","Driver":"local"});
    let vol = c.volume_create(cfg).await.expect("create volume");

    let inspected = c
        .volume_inspect("ewe-it-nv-inspect")
        .await
        .expect("inspect volume");

    assert_eq!(inspected.name, "ewe-it-nv-inspect");
    assert_eq!(inspected.driver, "local");

    c.volume_delete(&vol.name, Some(true))
        .await
        .expect("delete volume");
}

#[valtron_test]
async fn volume_prune() {
    let c = client();
    c.volume_prune(None).await.expect("prune volumes");
}

// ── Exec tests ─────────────────────────────────────────────────────────

#[valtron_test]
async fn exec_inspect() {
    let c = client();

    let ctr = c
        .create_container(
            &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
            Some("ewe-it-nve-execinspect"),
        )
        .await
        .expect("create container");
    c.start_container(&ctr.id).await.expect("start");

    let exec_id = c
        .exec_create(&ctr.id, &["echo", "hi"])
        .await
        .expect("exec create");

    let inspected = c
        .exec_inspect(&exec_id.id)
        .await
        .expect("exec inspect");

    assert!(
        inspected.get("ID").is_some(),
        "exec inspect response should contain an ID field"
    );

    c.stop_container(&ctr.id, Some(5)).await.expect("stop");
    c.remove_container(&ctr.id, true).await.expect("remove");
}

#[valtron_test]
async fn exec_resize() {
    let c = client();

    let ctr = c
        .create_container(
            &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
            Some("ewe-it-nve-execresize"),
        )
        .await
        .expect("create container");
    c.start_container(&ctr.id).await.expect("start");

    let exec_id = c
        .exec_create(&ctr.id, &["sleep", "5"])
        .await
        .expect("exec create");

    // Start exec before resize — Docker returns 500 if exec hasn't been started
    let _ = c.exec_start(&exec_id.id, true).await;
    c.exec_resize(&exec_id.id, 40, 120)
        .await
        .expect("exec resize");

    c.stop_container(&ctr.id, Some(5)).await.expect("stop");
    c.remove_container(&ctr.id, true).await.expect("remove");
}

// ── System tests ───────────────────────────────────────────────────────

#[valtron_test]
async fn system_events() {
    let c = client();
    let _ = c
        .system_events(None, None, None)
        .await
        .expect("system events");
}
