//! The generated container application create and update calls (#911), and the
//! generated Worker version upload through `GeneratedCloudflareApi`.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;

use foundation_auth::server_credential::Secret;
use foundation_core::valtron::{block_on_future, initialize_pool};
use foundation_deployment_cloudflare::{
    Binding, ContainerDeployment, ContainerImage, DeploymentApi, DeploymentId, DeploymentStatus,
    DoMigration, GeneratedCloudflareApi, Route, WorkerDeployment,
};
use foundation_netio::HttpClientBuilder;

/// Reads one HTTP request: its head and the body of its `Content-Length`.
fn read_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut data = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = stream.read(&mut buffer).expect("the request reads");
        if read == 0 {
            return data;
        }
        data.extend_from_slice(&buffer[..read]);
        let Some(head_end) = find(&data, b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&data[..head_end + 4]).to_lowercase();
        let length = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if data.len() >= head_end + 4 + length {
            return data;
        }
    }
}

/// The first position of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// A server that answers `responses` in order and returns every request.
fn mock_server(
    responses: Vec<(u16, String)>,
) -> (SocketAddr, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("the test socket");
    let address = listener.local_addr().expect("the test address");
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().expect("the test connection");
            requests.push(String::from_utf8_lossy(&read_request(&mut stream)).to_string());
            let reason = if status == 200 { "OK" } else { "Not Found" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\nconnection: close\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("the response writes");
        }
        requests
    });
    (address, handle)
}

/// The envelope of one successful Cloudflare call.
fn envelope(result: &str) -> String {
    format!(r#"{{"success":true,"errors":[],"messages":[],"result":{result}}}"#)
}

/// The full JSON of one container application.
fn application(name: &str, image: &str) -> String {
    format!(
        r#"{{"created_at":"2026-01-01T00:00:00Z","health":{{"active":0,"assigned":0}},"id":"app-1","image":"{image}","instance_type":"lite","max_instances":10,"name":"{name}","updated_at":"2026-01-01T00:00:00Z","version":1}}"#
    )
}

/// The envelope of one application.
fn one_application(name: &str, image: &str) -> String {
    format!(
        r#"{{"success":true,"errors":[],"messages":[],"result":{}}}"#,
        application(name, image)
    )
}

/// An envelope whose result is an empty list.
fn no_applications() -> String {
    r#"{"success":true,"errors":[],"messages":[],"result":[]}"#.to_string()
}

/// An envelope whose result is one application.
fn list_with_application(name: &str, image: &str) -> String {
    format!(
        r#"{{"success":true,"errors":[],"messages":[],"result":[{}]}}"#,
        application(name, image)
    )
}

/// The container of the test plan.
fn container() -> ContainerDeployment {
    ContainerDeployment {
        class_name: "SandboxDo".to_string(),
        name: "ewe-sandbox".to_string(),
        image: ContainerImage::Dockerfile(PathBuf::from("Dockerfile")),
        max_instances: 10,
        instance_type: "lite".to_string(),
    }
}

/// A create call reaches the generated create function.
#[test]
fn ensure_container_creates_a_missing_application() {
    let (address, server) = mock_server(vec![
        (200, no_applications()),
        (200, one_application("ewe-sandbox", "tag-1")),
    ]);
    let api = GeneratedCloudflareApi::new(
        HttpClientBuilder::new().build(),
        "account",
        "ewe-sandbox",
        format!("http://{address}"),
        Secret::new("token"),
    );
    let _pool = initialize_pool(17, Some(3));
    let application = container();
    block_on_future(async move {
        api.ensure_container("ewe-sandbox", &application, "tag-1")
            .await
    })
    .expect("the create");

    let requests = server.join().expect("the server thread");
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0].starts_with("GET /accounts/account/containers?name=ewe-sandbox"),
        "first request: {:?}",
        requests[0]
    );
    assert!(requests[0].contains("name=ewe-sandbox"));
    assert!(requests[1].starts_with("POST /accounts/account/containers/applications "));
    assert!(requests[1].contains(r#""name":"ewe-sandbox""#));
    assert!(requests[1].contains(r#""image":"tag-1""#));
    assert!(requests[1].contains(r#""instance_type":"lite""#));
    assert!(requests[1].contains(r#""max_instances":10"#));
    assert!(requests[1]
        .to_lowercase()
        .contains("authorization: bearer token"));
}

/// An update call reaches the generated update function.
#[test]
fn ensure_container_updates_an_existing_application() {
    let (address, server) = mock_server(vec![
        (200, list_with_application("ewe-sandbox", "tag-1")),
        (200, one_application("ewe-sandbox", "tag-2")),
    ]);
    let api = GeneratedCloudflareApi::new(
        HttpClientBuilder::new().build(),
        "account",
        "ewe-sandbox",
        format!("http://{address}"),
        Secret::new("token"),
    );
    let _pool = initialize_pool(18, Some(3));
    let application = container();
    block_on_future(async move {
        api.ensure_container("ewe-sandbox", &application, "tag-2")
            .await
    })
    .expect("the update");

    let requests = server.join().expect("the server thread");
    assert!(requests[1].starts_with("PATCH /accounts/account/containers/applications/app-1 "));
    assert!(requests[1].contains(r#""image":"tag-2""#));
}

/// The version upload uses the generated multipart call and returns its id.
#[test]
fn upload_version_carries_the_module() {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ewe-container-api-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&directory).expect("the test directory");
    let main = directory.join("index.js");
    std::fs::write(&main, b"export default {}").expect("the test module");
    let (address, server) = mock_server(vec![(200, envelope(r#"{"id":"v1","resources":{}}"#))]);
    let api = GeneratedCloudflareApi::new(
        HttpClientBuilder::new().build(),
        "account",
        "ewe-sandbox",
        format!("http://{address}"),
        Secret::new("token"),
    );
    let worker = WorkerDeployment {
        name: "ewe-sandbox".to_string(),
        main,
        compatibility_date: "2026-05-12".to_string(),
    };
    let bindings = vec![Binding::Ai {
        name: "AI".to_string(),
    }];
    let migrations = vec![DoMigration {
        tag: "v1".to_string(),
        new_sqlite_classes: vec!["SandboxDo".to_string()],
    }];
    let _pool = initialize_pool(19, Some(3));
    let version =
        block_on_future(async move { api.upload_version(&worker, &bindings, &migrations).await })
            .expect("the upload");
    assert_eq!(version, "v1");

    let requests = server.join().expect("the server thread");
    assert!(requests[0].starts_with("POST /accounts/account/workers/scripts/ewe-sandbox/versions "));
    assert!(requests[0].contains("name=\"metadata\""));
    assert!(requests[0].contains(r#""main_module":"index.js""#));
    assert!(
        requests[0].contains(r#""migrations":[{"new_sqlite_classes":["SandboxDo"],"tag":"v1"}]"#),
        "the metadata carries the migrations: {:?}",
        requests[0]
    );
    assert!(requests[0].contains("name=\"files\"; filename=\"index.js\""));
    assert!(requests[0].contains("export default {}"));
}

/// One route create call carries the pattern and the script.
#[test]
fn set_route_creates_the_route() {
    let (address, server) = mock_server(vec![(
        200,
        envelope(r#"{"id":"route-1","pattern":"sandbox.example.com/*","script":"ewe-sandbox"}"#),
    )]);
    let api = GeneratedCloudflareApi::new(
        HttpClientBuilder::new().build(),
        "account",
        "ewe-sandbox",
        format!("http://{address}"),
        Secret::new("token"),
    );
    let _pool = initialize_pool(20, Some(3));
    let route = Route {
        pattern: "sandbox.example.com/*".to_string(),
        zone: Some("zone-1".to_string()),
    };
    block_on_future(async move { api.set_route("ewe-sandbox", &route).await }).expect("the route");

    let requests = server.join().expect("the server thread");
    assert!(requests[0].starts_with("POST /zones/zone-1/workers/routes "));
    assert!(requests[0].contains(r#""pattern":"sandbox.example.com/*""#));
    assert!(requests[0].contains(r#""script":"ewe-sandbox""#));
}

/// The traffic switch posts the version at 100 percent, then reads the account
/// subdomain for the Worker address.
#[test]
fn promote_switches_traffic() {
    let (address, server) = mock_server(vec![
        (
            200,
            envelope(
                r#"{"id":"deployment-1","created_on":"2026-01-01T00:00:00Z","source":"api","strategy":"percentage","versions":[]}"#,
            ),
        ),
        (200, envelope(r#"{"subdomain":"example"}"#)),
    ]);
    let api = GeneratedCloudflareApi::new(
        HttpClientBuilder::new().build(),
        "account",
        "ewe-sandbox",
        format!("http://{address}"),
        Secret::new("token"),
    );
    let _pool = initialize_pool(21, Some(3));
    let promotion = block_on_future(async move { api.promote("ewe-sandbox", "v1", None).await })
        .expect("the promote");
    assert_eq!(promotion.deployment_id.as_str(), "deployment-1");
    assert_eq!(
        promotion.worker_url,
        "https://ewe-sandbox.example.workers.dev"
    );

    let requests = server.join().expect("the server thread");
    assert!(
        requests[0].starts_with("POST /accounts/account/workers/scripts/ewe-sandbox/deployments ")
    );
    assert!(requests[0].contains(r#""strategy":"percentage""#));
    assert!(requests[0].contains(r#""version_id":"v1""#));
    assert!(requests[0].contains(r#""percentage":100"#));
    assert!(
        requests[1].starts_with("GET /accounts/account/workers/subdomain "),
        "the promote reads the account subdomain: {}",
        requests[1]
    );
}

/// A refused subdomain read falls back to the bare workers.dev address.
#[test]
fn promote_without_subdomain_falls_back() {
    let (address, server) = mock_server(vec![
        (
            200,
            envelope(
                r#"{"id":"deployment-1","created_on":"2026-01-01T00:00:00Z","source":"api","strategy":"percentage","versions":[]}"#,
            ),
        ),
        (404, "{}".to_string()),
    ]);
    let api = GeneratedCloudflareApi::new(
        HttpClientBuilder::new().build(),
        "account",
        "ewe-sandbox",
        format!("http://{address}"),
        Secret::new("token"),
    );
    let _pool = initialize_pool(23, Some(3));
    let promotion = block_on_future(async move { api.promote("ewe-sandbox", "v1", None).await })
        .expect("the promote");
    assert_eq!(promotion.worker_url, "https://ewe-sandbox.workers.dev");

    let requests = server.join().expect("the server thread");
    assert_eq!(requests.len(), 2, "the deployment and the subdomain read");
    assert!(
        requests[1].starts_with("GET /accounts/account/workers/subdomain "),
        "the subdomain read happens before the fallback: {}",
        requests[1]
    );
}

/// A plan route is the Worker address, so the promote reads no subdomain.
#[test]
fn promote_uses_the_first_route() {
    let (address, server) = mock_server(vec![(
        200,
        envelope(
            r#"{"id":"deployment-1","created_on":"2026-01-01T00:00:00Z","source":"api","strategy":"percentage","versions":[]}"#,
        ),
    )]);
    let api = GeneratedCloudflareApi::new(
        HttpClientBuilder::new().build(),
        "account",
        "ewe-sandbox",
        format!("http://{address}"),
        Secret::new("token"),
    );
    let _pool = initialize_pool(24, Some(3));
    let route = Route {
        pattern: "sandbox.example.com/*".to_string(),
        zone: Some("zone-1".to_string()),
    };
    let promotion =
        block_on_future(async move { api.promote("ewe-sandbox", "v1", Some(&route)).await })
            .expect("the promote");
    assert_eq!(promotion.worker_url, "https://sandbox.example.com");

    let requests = server.join().expect("the server thread");
    assert_eq!(requests.len(), 1, "a plan route skips the subdomain read");
    assert!(
        requests[0].starts_with("POST /accounts/account/workers/scripts/ewe-sandbox/deployments ")
    );
}

/// A status read finds the live deployment, and a missing one is a rollback.
#[test]
fn status_reads_the_deployment() {
    let (address, server) = mock_server(vec![
        (
            200,
            envelope(
                r#"{"id":"deployment-1","created_on":"2026-01-01T00:00:00Z","source":"api","strategy":"percentage","versions":[]}"#,
            ),
        ),
        (404, "{}".to_string()),
    ]);
    let api = GeneratedCloudflareApi::new(
        HttpClientBuilder::new().build(),
        "account",
        "ewe-sandbox",
        format!("http://{address}"),
        Secret::new("token"),
    );
    let _pool = initialize_pool(22, Some(3));
    let id = DeploymentId::new("deployment-1");
    let live = block_on_future({
        let api = api.clone();
        let id = id.clone();
        async move { api.status(&id).await }
    })
    .expect("the status");
    assert_eq!(live, DeploymentStatus::Live);
    let gone = block_on_future(async move { api.status(&id).await }).expect("the missing status");
    assert_eq!(gone, DeploymentStatus::RolledBack);

    let requests = server.join().expect("the server thread");
    assert!(requests[0].starts_with(
        "GET /accounts/account/workers/scripts/ewe-sandbox/deployments/deployment-1 "
    ));
    assert!(requests[1].starts_with(
        "GET /accounts/account/workers/scripts/ewe-sandbox/deployments/deployment-1 "
    ));
}

/// A rollback deletes the deployment, and a second rollback is `Ok`.
#[test]
fn rollback_deletes_the_deployment() {
    let (address, server) = mock_server(vec![(200, envelope("null")), (404, "{}".to_string())]);
    let api = GeneratedCloudflareApi::new(
        HttpClientBuilder::new().build(),
        "account",
        "ewe-sandbox",
        format!("http://{address}"),
        Secret::new("token"),
    );
    let _pool = initialize_pool(23, Some(3));
    let id = DeploymentId::new("deployment-1");
    block_on_future(async move {
        api.rollback(&id).await?;
        api.rollback(&id).await
    })
    .expect("both rollbacks succeed");

    let requests = server.join().expect("the server thread");
    assert!(requests[0].starts_with(
        "DELETE /accounts/account/workers/scripts/ewe-sandbox/deployments/deployment-1 "
    ));
    assert!(requests[1].starts_with(
        "DELETE /accounts/account/workers/scripts/ewe-sandbox/deployments/deployment-1 "
    ));
}
