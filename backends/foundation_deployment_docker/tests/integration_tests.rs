//! Integration tests against a running Docker daemon.
//!
//! WHY: Real docker daemon, no mocks.
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test integration_tests -- --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

fn c() -> DockerClient { DockerClient::connect_unix("/var/run/docker.sock") }

// ── Active: read-only + simple create-then-delete ─────────────────────

#[valtron_test] async fn ping() { c().system_ping().await.expect("ping"); }
#[valtron_test] async fn info() { assert!(c().system_info().await.expect("info").id.is_some() || c().system_info().await.expect("info").name.is_some()); }
#[valtron_test] async fn df() { assert!(!c().system_data_usage(None).await.expect("df").is_null()); }
#[valtron_test] async fn net_list() { assert!(c().network_list(None).await.expect("list").is_array()); }
#[valtron_test] async fn vol_list() { let _ = c().volume_list(None).await.expect("list"); }
#[valtron_test] async fn img_list() { assert!(c().image_list(Some(true),None,None,None).await.expect("list").is_array()); }
#[valtron_test] async fn img_search() { assert!(c().image_search(Some("alpine"),None,None).await.expect("search").is_array()); }
#[valtron_test] async fn ctr_list() { assert!(c().container_list(true,None,false,None).await.expect("list").is_array()); }

#[valtron_test]
async fn net_create_delete() {
    let body = serde_json::json!({"Name":"ewe-it-net","Driver":"bridge"});
    let cr = c().network_create(body).await.expect("create");
    assert!(!cr.id.is_empty());
    c().network_delete(&cr.id).await.expect("delete");
}

#[valtron_test]
async fn vol_create_delete() {
    let cfg = serde_json::json!({"Name":"ewe-it-vol","Driver":"local"});
    let vol = c().volume_create(cfg).await.expect("create");
    assert!(!vol.name.is_empty());
    c().volume_delete(&vol.name, Some(true)).await.expect("delete");
}

// ── Ignored: multi-call lifecycle (netio connection-reuse bug) ────────

#[valtron_test]
async fn container_lifecycle_create_start_stop_remove() {
    let id = c().create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","60"]}),
        Some("ewe-it-lifecycle"),
    ).await.expect("create");
    assert!(!id.id.is_empty());
    c().start_container(&id.id).await.expect("start");
    let inspect = c().inspect_container(&id.id).await.expect("inspect");
    assert!(inspect.state.as_ref().and_then(|s|s.running).unwrap_or(false));
    c().stop_container(&id.id, Some(10)).await.expect("stop");
    c().remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_stop_already_exited_handles_304() {
    let id = c().create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["true"]}),
        Some("ewe-it-304"),
    ).await.expect("create");
    c().start_container(&id.id).await.expect("start");
    std::thread::sleep(std::time::Duration::from_secs(2));
    c().stop_container(&id.id, Some(5)).await.expect("stop (304)");
    c().remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_restart_cycles() {
    let id = c().create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
        Some("ewe-it-restart"),
    ).await.expect("create");
    c().start_container(&id.id).await.expect("start");
    c().restart_container(&id.id, None, None).await.expect("restart");
    c().stop_container(&id.id, Some(5)).await.expect("stop");
    c().remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_rename_changes_name() {
    let id = c().create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","5"]}),
        Some("ewe-it-original"),
    ).await.expect("create");
    c().rename_container(&id.id, "ewe-it-renamed").await.expect("rename");
    let _ = c().stop_container(&id.id, Some(1)).await;
    c().remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_logs_returns_output() {
    let id = c().create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["echo","hello-it-test"]}),
        Some("ewe-it-logs"),
    ).await.expect("create");
    c().start_container(&id.id).await.expect("start");
    std::thread::sleep(std::time::Duration::from_secs(2));
    let logs = c().container_logs(&id.id, false, true, false, None, None, false, None).await.expect("logs");
    assert!(std::str::from_utf8(&logs).unwrap_or("").contains("hello-it-test"));
    c().remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_top_shows_processes() {
    let id = c().create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
        Some("ewe-it-top"),
    ).await.expect("create");
    c().start_container(&id.id).await.expect("start");
    let top = c().container_top(&id.id, None).await.expect("top");
    assert!(top.titles.is_some());
    c().stop_container(&id.id, Some(5)).await.expect("stop");
    c().remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn exec_create_start_returns_output() {
    let id = c().create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
        Some("ewe-it-exec"),
    ).await.expect("create");
    c().start_container(&id.id).await.expect("start");
    let exec_id = c().exec_create(&id.id, &["echo","exec-output"]).await.expect("exec create");
    assert!(!exec_id.id.is_empty());
    let output = c().exec_start(&exec_id.id, false).await.expect("exec start");
    assert!(std::str::from_utf8(&output).unwrap_or("").contains("exec-output"));
    c().stop_container(&id.id, Some(5)).await.expect("stop");
    c().remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn negotiate_version_detects_real_server() {
    let mut client = c();
    client.negotiate_version().await.expect("negotiate");
    assert!(client.api_version() == "1.54" || client.api_version() == "1.53");
}

#[valtron_test]
async fn cleanup_leftover_containers() {
    let client = c();
    let list = client.container_list(true, None, false, None).await.expect("list");
    if let Some(arr) = list.as_array() {
        for ctr in arr {
            let name = ctr.get("Names").and_then(|n|n.as_array()).and_then(|a|a.first()).and_then(|n|n.as_str()).unwrap_or("");
            if name.starts_with("/ewe-it-") {
                if let Some(id) = ctr.get("Id").and_then(|i|i.as_str()) {
                    let _ = client.remove_container(id, true).await;
                }
            }
        }
    }
}

#[valtron_test]
async fn repro_stop_then_remove_fresh_client_per_call() {
    // Create + start
    let c1 = DockerClient::connect_unix("/var/run/docker.sock");
    let id = c1.create_container(
        &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
        Some("ewe-repro1"),
    ).await.expect("create");
    c1.start_container(&id.id).await.expect("start");

    // Stop — fresh client (new connection)
    let c2 = DockerClient::connect_unix("/var/run/docker.sock");
    c2.stop_container(&id.id, Some(5)).await.expect("stop");

    // Remove — fresh client (new connection)
    let c3 = DockerClient::connect_unix("/var/run/docker.sock");
    c3.remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn repro_stop_then_remove_same_client() {
    // Same DockerClient for all calls — this is what hangs
    let c = DockerClient::connect_unix("/var/run/docker.sock");
    let id = c.create_container(
        &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
        Some("ewe-repro2"),
    ).await.expect("create");
    c.start_container(&id.id).await.expect("start");
    c.stop_container(&id.id, Some(5)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
}
