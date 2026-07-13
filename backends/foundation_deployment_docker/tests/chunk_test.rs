#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]
use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;
use foundation_netio::{DynNetClient, PreparedRequestBuilder};
use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;

#[valtron_test]
async fn trace_chunked_logs() {
    let c = DockerClient::connect_unix("/var/run/docker.sock");
    
    // Create container that outputs something then sleeps
    let id = c.create_container(
        &serde_json::json!({"Image":"alpine:latest","Cmd":["sh","-c","echo hi; sleep 30"]}),
        Some("ewe-it-logs"),
    ).await.expect("create");
    eprintln!("TRACE1: created {}", id.id);
    
    c.start_container(&id.id).await.expect("start");
    eprintln!("TRACE2: started");
    
    // Small delay so echo completes
    std::thread::sleep(std::time::Duration::from_secs(1));
    eprintln!("TRACE3: waited 1s");

    let http: DynNetClient = c.http();
    let base = c.base_url();
    let url = format!("{}/containers/{}/logs", base, id.id);
    eprintln!("TRACE4: URL={}", url);
    
    let builder = PreparedRequestBuilder::get(&url).unwrap()
        .query("stdout", Some("1"))
        .query("stderr", Some("0"))
        .query("follow", Some("0"));
    eprintln!("TRACE5: builder ready");
    
    let resp = http.send_async(builder.build()).await.expect("send_async");
    eprintln!("TRACE6: send_async OK, status={}", resp.get_status());
    
    let body_type = resp.get_body_ref();
    eprintln!("TRACE7: body_ref type");
    
    let body = resp.take_body();
    eprintln!("TRACE8: take_body done, variant={:?}", 
        if matches!(body, foundation_netio::shared::http::SendSafeBody::None(_)) { "None" }
        else if matches!(body, foundation_netio::shared::http::SendSafeBody::Bytes(_)) { "Bytes" }
        else if matches!(body, foundation_netio::shared::http::SendSafeBody::Stream(_)) { "Stream" }
        else if matches!(body, foundation_netio::shared::http::SendSafeBody::ChunkedStream(_)) { "ChunkedStream" }
        else { "other" }
    );
    
    eprintln!("TRACE9: about to call collect_bytes_from_send_safe...");
    let bytes = collect_bytes_from_send_safe(body);
    eprintln!("TRACE10: collected {} bytes: {:?}", bytes.len(), &bytes[..bytes.len().min(64)]);
    
    c.stop_container(&id.id, Some(5)).await.expect("stop");
    c.remove_container(&id.id, true).await.expect("remove");
    eprintln!("TRACE11: DONE");
}
