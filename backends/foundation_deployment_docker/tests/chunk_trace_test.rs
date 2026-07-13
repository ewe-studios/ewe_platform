//! Traced chunked transfer-encoding integration test for container_logs.
//!
//! WHY: Docker returns `Transfer-Encoding: chunked` for logs responses — proving
//! `SimpleHttpChunkIterator` drains the stream end-to-end against a real daemon.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test chunk_trace_test -- --nocapture --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;
use foundation_netio::{DynNetClient, PreparedRequestBuilder};
use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;
use tracing_test::traced_test;

#[traced_test]
#[valtron_test]
async fn container_logs_chunked_body_round_trip() {
    let c = DockerClient::connect_unix("/var/run/docker.sock");

    let id = c.create_container(
        &serde_json::json!({"Image": "alpine:latest", "Cmd": ["echo", "hello-chunked-test"]}),
        Some("ewe-it-chunked"),
    ).await.expect("create");
    tracing::info!(id = %id.id, "container created");

    c.start_container(&id.id).await.expect("start");
    tracing::info!("container started");
    std::thread::sleep(std::time::Duration::from_secs(2));
    tracing::info!("waited for echo to complete");

    // Use the raw send_async path that produces a ChunkedStream body
    let http: DynNetClient = c.http();
    let url = format!("{}/containers/{}/logs", c.base_url(), id.id);
    let builder = PreparedRequestBuilder::get(&url)
        .unwrap()
        .query("stdout", Some("1"))
        .query("stderr", Some("0"))
        .query("follow", Some("0"));

    tracing::info!(%url, "sending logs request");
    let resp = http.send_async(builder.build()).await.expect("send_async");
    let status = resp.get_status();
    tracing::info!(%status, "received response");

    let body = resp.take_body();
    // body is SendSafeBody::ChunkedStream when Transfer-Encoding: chunked
    tracing::info!("calling collect_bytes_from_send_safe");
    let bytes = collect_bytes_from_send_safe(body);
    tracing::info!(len = bytes.len(), "collected bytes");

    assert!(!bytes.is_empty(), "logs should return data");
    assert!(
        std::str::from_utf8(&bytes).unwrap_or("").contains("hello-chunked-test"),
        "should contain echoed text: {}",
        String::from_utf8_lossy(&bytes)
    );

    c.remove_container(&id.id, true).await.expect("remove");
    tracing::info!("DONE");
}
