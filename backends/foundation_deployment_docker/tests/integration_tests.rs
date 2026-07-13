//! Integration tests against a running Docker daemon.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test integration_tests -- --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;
use tracing_test::traced_test;

/// One fresh client per test — no shared state, no pool interference.
fn client() -> DockerClient { DockerClient::connect_unix("/var/run/docker.sock") }

// ── Read-only: prove transport + serde work against real dockerd ──────

#[valtron_test] async fn ping()     { client().system_ping().await.expect("ping"); }
#[valtron_test] async fn info()     { let _ = client().system_info().await.expect("info"); }
#[valtron_test] async fn df()       { let _ = client().system_data_usage(None).await.expect("df"); }
#[valtron_test] async fn net_list() { let _ = client().network_list(None).await.expect("net list"); }
#[valtron_test] async fn vol_list() { let _ = client().volume_list(None).await.expect("vol list"); }
#[valtron_test] async fn img_list() { let _ = client().image_list(Some(true),None,None,None).await.expect("img list"); }
#[valtron_test] async fn img_search(){ let _ = client().image_search(Some("alpine"),None,None).await.expect("search"); }
#[valtron_test] async fn ctr_list() { let _ = client().container_list(true,None,false,None).await.expect("ctr list"); }

// ── Create-then-delete: simple lifecycle, no blocking stop ────────────

#[valtron_test]
async fn network_create_then_delete() {
    let c = client();
    let body = serde_json::json!({"Name":"ewe-it-net","Driver":"bridge"});
    let cr = c.network_create(body).await.expect("create");
    c.network_delete(&cr.id).await.expect("delete");
}

#[valtron_test]
async fn volume_create_then_delete() {
    let c = client();
    let cfg = serde_json::json!({"Name":"ewe-it-vol","Driver":"local"});
    let vol = c.volume_create(cfg).await.expect("create");
    c.volume_delete(&vol.name, Some(true)).await.expect("delete");
}

// ── Container lifecycle: one client per test, stop while running ──────

#[valtron_test]
async fn container_full_lifecycle() {
    let c = client();
    let id = c.create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
        Some("ewe-it-lifecycle"),
    ).await.expect("create");
    c.start_container(&id.id).await.expect("start");
    c.stop_container(&id.id, Some(10)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_stop_handles_304() {
    let c = client();
    let id = c.create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["true"]}),
        Some("ewe-it-304"),
    ).await.expect("create");
    c.start_container(&id.id).await.expect("start");
    std::thread::sleep(std::time::Duration::from_secs(2));
    c.stop_container(&id.id, Some(5)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_restart_works() {
    let c = client();
    let id = c.create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
        Some("ewe-it-restart"),
    ).await.expect("create");
    c.start_container(&id.id).await.expect("start");
    c.restart_container(&id.id, None, None).await.expect("restart");
    c.stop_container(&id.id, Some(5)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn container_rename_works() {
    let c = client();
    let id = c.create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","5"]}),
        Some("ewe-it-original"),
    ).await.expect("create");
    c.rename_container(&id.id, "ewe-it-renamed").await.expect("rename");
    // Container exits naturally after 5s — stop may return 304
    let _ = c.stop_container(&id.id, Some(2)).await;
    c.remove_container(&id.id, true).await.expect("remove");
}



#[valtron_test]
async fn container_top_shows_processes() {
    let c = client();
    let id = c.create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
        Some("ewe-it-top"),
    ).await.expect("create");
    c.start_container(&id.id).await.expect("start");
    let top = c.container_top(&id.id, None).await.expect("top");
    assert!(top.titles.is_some());
    c.stop_container(&id.id, Some(5)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn exec_create_start_returns_output() {
    let c = client();
    let id = c.create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
        Some("ewe-it-exec"),
    ).await.expect("create");
    c.start_container(&id.id).await.expect("start");
    let exec_id = c.exec_create(&id.id, &["echo","exec-ok"]).await.expect("exec create");
    let output = c.exec_start(&exec_id.id, false).await.expect("exec start");
    assert!(std::str::from_utf8(&output).unwrap_or("").contains("exec-ok"));
    c.stop_container(&id.id, Some(5)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn negotiate_version_detects_server() {
    let mut c = client();
    c.negotiate_version().await.expect("negotiate");
    let v = c.api_version();
    assert!(v == "1.53" || v == "1.54", "version is {v}");
}

#[valtron_test]
async fn cleanup_leftovers() {
    let c = client();
    let list = c.container_list(true,None,false,None).await.expect("list");
    if let Some(arr) = list.as_array() {
        for ctr in arr {
            let name = ctr.get("Names").and_then(|n|n.as_array()).and_then(|a|a.first()).and_then(|n|n.as_str()).unwrap_or("");
            if name.starts_with("/ewe-it-") {
                if let Some(id) = ctr.get("Id").and_then(|i|i.as_str()) {
                    let _ = c.remove_container(id, true).await;
                }
            }
        }
    }
}

#[valtron_test]
#[traced_test]
async fn container_logs_chunked_body_round_trip() {
    let c = client();
    let id = c.create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["echo","hello-chunked-test"]}),
        Some("ewe-it-chunked"),
    ).await.expect("create");
    tracing::info!(id = %id.id, "created");

    c.start_container(&id.id).await.expect("start");
    tracing::info!("started");
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Use the raw send_async path + collect_bytes_from_send_safe
    let http = c.http();
    let url = format!("{}/containers/{}/logs", c.base_url(), id.id);
    let builder = foundation_netio::PreparedRequestBuilder::get(&url)
        .unwrap()
        .query("stdout", Some("1"))
        .query("stderr", Some("0"))
        .query("follow", Some("0"));
    tracing::info!(%url, "sending");

    let resp = http.send_async(builder.build()).await.expect("send_async");
    tracing::info!(status = %resp.get_status(), "response");

    let body = resp.take_body();
    let bytes = foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe(body);
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(128)]);
    tracing::info!(len = bytes.len(), text = %text, "collected");

    assert!(std::str::from_utf8(&bytes).unwrap_or("").contains("hello-chunked-test"),
        "should contain our echo output");

    c.remove_container(&id.id, true).await.expect("remove");
    tracing::info!("DONE");
}
