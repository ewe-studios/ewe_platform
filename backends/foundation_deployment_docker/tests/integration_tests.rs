//! Integration tests against a running Docker daemon.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test integration_tests -- --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

mod common;

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

/// One fresh client per test — no shared state, no pool interference.
/// A client for the local daemon, with `alpine:latest` pulled (the tests
/// create containers from it; a cold daemon does not have it).
async fn client() -> DockerClient {
    common::client_with_alpine().await
}

// ── Read-only: prove transport + serde work against real dockerd ──────

#[valtron_test] async fn ping()     { client().await.system_ping().await.expect("ping"); }
#[valtron_test] async fn info()     { let i = client().await.system_info().await.expect("info"); assert!(!i.is_null(), "SystemInfo should be valid JSON"); }
#[valtron_test] async fn df()       { let _ = client().await.system_data_usage(None).await.expect("df"); }
#[valtron_test] async fn net_list() { let _ = client().await.network_list(None).await.expect("net list"); }
#[valtron_test] async fn vol_list() { let _ = client().await.volume_list(None).await.expect("vol list"); }
#[valtron_test] async fn img_list() { let _ = client().await.image_list(Some(true),None,None,None).await.expect("img list"); }
#[valtron_test] async fn img_search(){ let _ = client().await.image_search(Some("alpine"),None,None).await.expect("search"); }
#[valtron_test] async fn ctr_list() { let _ = client().await.container_list(true,None,false,None).await.expect("ctr list"); }

// ── Create-then-delete: simple lifecycle, no blocking stop ────────────

#[valtron_test]
async fn network_create_then_delete() {
    let c = client().await;
    let body = serde_json::json!({"Name":"ewe-it-net","Driver":"bridge"});
    let cr = c.network_create(body).await.expect("create");
    c.network_delete(&cr.id).await.expect("delete");
}

#[valtron_test]
async fn volume_create_then_delete() {
    let c = client().await;
    let cfg = serde_json::json!({"Name":"ewe-it-vol","Driver":"local"});
    let vol = c.volume_create(cfg).await.expect("create");
    c.volume_delete(&vol.name, Some(true)).await.expect("delete");
}

// ── Container lifecycle: one client per test, stop while running ──────

#[valtron_test]
async fn container_full_lifecycle() {
    let c = client().await;
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
    // Docker returns 304 when the container is already exited.
    // Fixed: send_request was misclassifying 304 as redirect causing
    // infinite redirect loops with no Location header.
    let c = client().await;
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
    let c = client().await;
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
    let c = client().await;
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
    let c = client().await;
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
    let c = client().await;
    let id = c.create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sleep","30"]}),
        Some("ewe-it-exec"),
    ).await.expect("create");
    c.start_container(&id.id).await.expect("start");
    let exec_id = c.exec_create(&id.id, &["echo","exec-ok"]).await.expect("exec create");
    let (mut stdout, mut stderr) = (String::new(), String::new());
    c.exec_start(&exec_id.id, false)
        .await
        .expect("exec start")
        .read_into(&mut stdout, &mut stderr)
        .await
        .expect("exec output");
    assert!(stdout.contains("exec-ok"), "stdout: {stdout:?}, stderr: {stderr:?}");
    c.stop_container(&id.id, Some(5)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
}

#[valtron_test]
async fn negotiate_version_detects_server() {
    let c = client().await;
    c.negotiate_version().await.expect("negotiate");
    let daemon = c.version().await.expect("version");
    let expected = foundation_deployment_docker::client::negotiate_api_version(
        foundation_deployment_docker::client::DEFAULT_API_VERSION,
        daemon.api_version.as_deref(),
        daemon.min_api_version.as_deref(),
    )
    .expect("this daemon shares an API version with the client");
    assert_eq!(c.api_version(), expected, "min(client max, daemon ApiVersion)");
    // A versioned request with the negotiated version is accepted.
    c.system_ping().await.expect("ping with the negotiated version");
}

#[valtron_test]
async fn cleanup_leftovers() {
    let c = client().await;
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

// NOTE: no `#[traced_test]` here — `#[valtron_test]` already installs a tracing
// subscriber, and `traced_test`'s `set_global_default().expect(..)` panics with
// `SetGlobalDefaultError` once any earlier test in the binary has set the global
// default (fine alone, fails in the suite). The `tracing::*` calls below still
// route through valtron_test's subscriber.
#[valtron_test]
async fn container_logs_chunked_body_round_trip() {
    let c = client().await;
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
