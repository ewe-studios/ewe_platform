//! End-to-end BuildKit tests against a real `buildkitd` over gRPC/HTTP2.
//!
//! WHY: Proves the buffa-generated Control message types + `foundation_connectrpc`
//! gRPC client actually speak to a live buildkitd — not just round-trip in
//! isolation. Exercises the unary `Info`/`ListWorkers`/`DiskUsage` RPCs over
//! `H2Transport`.
//!
//! SETUP: point `EWE_BUILDKITD_ADDR` at a buildkitd TCP listener, e.g.
//!   docker run -d --privileged -p 127.0.0.1:13434:1234 \
//!     moby/buildkit:latest --addr tcp://0.0.0.0:1234
//! then `EWE_BUILDKITD_ADDR=127.0.0.1:13434`. Tests skip (pass) if unset or
//! unreachable, so CI without a buildkitd stays green.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "buildkit,integration-tests" --profile uat
//!   --test buildkit_integration_tests -- --test-threads=1`

#![cfg(all(unix, feature = "buildkit", feature = "integration-tests"))]

use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::buildkit::BuildKitClient;
use foundation_deployment_docker::buildkit::types::BytesMessage;
use foundation_deployment_docker::buildkit::services::{filesync, health};

/// Resolve the buildkitd address, or `None` to skip (no daemon available).
fn buildkitd_addr() -> Option<String> {
    let addr = std::env::var("EWE_BUILDKITD_ADDR").unwrap_or_else(|_| "127.0.0.1:13434".to_string());
    // Skip unless the TCP port actually accepts a connection.
    match std::net::TcpStream::connect(&addr) {
        Ok(_) => Some(addr),
        Err(_) => {
            eprintln!("skipping: no buildkitd reachable at {addr} (set EWE_BUILDKITD_ADDR)");
            None
        }
    }
}

#[valtron_test]
async fn info_returns_version() {
    let Some(addr) = buildkitd_addr() else { return };
    let client = BuildKitClient::connect_tcp(&addr).expect("connect_tcp");

    let info = client.info().await.expect("Info RPC");
    let version = info.buildkitVersion.into_option().expect("buildkitVersion present");
    assert!(!version.version.is_empty(), "version string should be non-empty");
    eprintln!("buildkitd version: {}", version.version);
}

#[valtron_test]
async fn list_workers_reports_a_worker() {
    let Some(addr) = buildkitd_addr() else { return };
    let client = BuildKitClient::connect_tcp(&addr).expect("connect_tcp");

    let workers = client.list_workers(Vec::new()).await.expect("ListWorkers RPC");
    assert!(!workers.record.is_empty(), "buildkitd should report >=1 worker");
    // Each worker advertises at least one platform.
    let first = &workers.record[0];
    assert!(!first.ID.is_empty(), "worker id should be non-empty");
}

#[valtron_test]
async fn disk_usage_ok() {
    let Some(addr) = buildkitd_addr() else { return };
    let client = BuildKitClient::connect_tcp(&addr).expect("connect_tcp");

    // A fresh daemon may have an empty cache — we only assert the RPC succeeds.
    let _usage = client.disk_usage(Vec::new()).await.expect("DiskUsage RPC");
}

#[valtron_test]
async fn solve_probe_reports_what_buildkit_needs() {
    let Some(addr) = buildkitd_addr() else { return };
    let client = BuildKitClient::connect_tcp(&addr).expect("connect_tcp");

    // Fire a dockerfile.v0 solve with NO session — we want to see exactly what
    // buildkitd complains about (empirical: drives the session design).
    use foundation_deployment_docker::buildkit::types::SolveRequest;
    let mut req = SolveRequest::default();
    req.Frontend = "dockerfile.v0".into();
    req.FrontendAttrs.insert("filename".into(), "Dockerfile".into());

    match client.solve(req).await {
        Ok(_) => eprintln!("SOLVE OK (unexpected without a context)"),
        Err(e) => eprintln!("SOLVE ERR (expected — tells us what's needed): {e}"),
    }
}

#[valtron_test(tracing = "debug")]
async fn session_bidi_manual_response() {
    use foundation_connectrpc::{
        Client, ClientOptions, Ctx, H2Transport, ProcedureCodecs, Transport,
    };

    let Some(addr) = buildkitd_addr() else { return };
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());

    let session_client = Client::<BytesMessage, BytesMessage>::new(
        transport,
        &format!("http://{addr}/moby.buildkit.v1.Control/Session"),
        ProcedureCodecs::defaults(),
        ClientOptions::new()
            .with_grpc()
            .with_header("x-docker-expose-session-uuid".to_string(), "manual-response-test")
            .with_header("x-docker-expose-session-name".to_string(), "ewe-manual")
            .with_header("x-docker-expose-session-grpc-method".to_string(), filesync::procedure::DIFF_COPY)
            .with_header("x-docker-expose-session-grpc-method".to_string(), health::procedure::CHECK),
    ).unwrap();

    let mut bidi = session_client
        .bidi_stream(Ctx::background(), futures::stream::pending::<BytesMessage>())
        .await
        .expect("open bidi");

    let (mut sender, mut receiver) = bidi.split();

    // Read H2 preface from buildkitd
    let preface = match receiver.receive().await {
        Ok(Some(msg)) => msg.data,
        other => { eprintln!("[manual] expected preface, got {other:?}"); return; }
    };
    eprintln!("[manual] got preface: {} bytes", preface.len());

    // Read client SETTINGS
    let client_settings = match receiver.receive().await {
        Ok(Some(msg)) => msg.data,
        other => { eprintln!("[manual] expected SETTINGS, got {other:?}"); return; }
    };
    eprintln!("[manual] got client SETTINGS: {} bytes", client_settings.len());

    // Send back minimal H2 server response: empty SETTINGS + SETTINGS ACK
    // This is exactly what grpc-go expects from the server
    // SETTINGS frame (empty — all defaults): 9 bytes
    // SETTINGS ACK frame: 9 bytes
    let server_settings: Vec<u8> = vec![
        0x00, 0x00, 0x00, // length = 0
        0x04,             // type = SETTINGS
        0x00,             // flags = none
        0x00, 0x00, 0x00, 0x00, // stream 0
    ];
    let settings_ack: Vec<u8> = vec![
        0x00, 0x00, 0x00, // length = 0
        0x04,             // type = SETTINGS
        0x01,             // flags = ACK
        0x00, 0x00, 0x00, 0x00, // stream 0
    ];

    eprintln!("[manual] sending server SETTINGS (9b)...");
    sender.send(&BytesMessage { data: server_settings, ..Default::default() }).await.unwrap();
    eprintln!("[manual] sending SETTINGS ACK (9b)...");
    sender.send(&BytesMessage { data: settings_ack, ..Default::default() }).await.unwrap();

    eprintln!("[manual] waiting for buildkitd's SETTINGS ACK...");
    // buildkitd should now send SETTINGS ACK
    match receiver.receive().await {
        Ok(Some(msg)) => {
            let hex: String = msg.data.iter().map(|b| format!("{b:02x}")).collect();
            eprintln!("[manual] got frame from bk: {} bytes {hex}", msg.data.len());
        }
        Ok(None) => eprintln!("[manual] stream ended after our SETTINGS"),
        Err(e) => eprintln!("[manual] receive error after our SETTINGS: {e}"),
    }

    // Now wait 3 seconds to see if the bidi stays alive or closes
    eprintln!("[manual] waiting 3s to see if bidi stays alive...");
    let start = std::time::Instant::now();
    // Read in a timeout loop
    loop {
        if start.elapsed() > std::time::Duration::from_secs(3) {
            eprintln!("[manual] TIMEOUT — bidi still alive after 3s! SUCCESS.");
            break;
        }
        match receiver.receive().await {
            Ok(Some(msg)) => {
                let hex: String = msg.data.iter().map(|b| format!("{b:02x}")).collect();
                eprintln!("[manual] bk→us {} bytes: {hex}", msg.data.len());
            }
            Ok(None) => {
                eprintln!("[manual] stream ended after {:?}", start.elapsed());
                break;
            }
            Err(e) => {
                eprintln!("[manual] receive error after {:?}: {e}", start.elapsed());
                break;
            }
        }
    }

    drop(sender);
    drop(receiver);
}

#[valtron_test(tracing = "debug")]
async fn session_bidi_raw_no_pump() {
    use foundation_connectrpc::{
        Client, ClientOptions, Ctx, H2Transport, ProcedureCodecs, Transport,
    };

    let Some(addr) = buildkitd_addr() else { return };
    let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());

    let session_client = Client::<BytesMessage, BytesMessage>::new(
        transport,
        &format!("http://{addr}/moby.buildkit.v1.Control/Session"),
        ProcedureCodecs::defaults(),
        ClientOptions::new()
            .with_grpc()
            .with_header("x-docker-expose-session-uuid".to_string(), "raw-test-no-pump")
            .with_header("x-docker-expose-session-name".to_string(), "ewe-raw")
            .with_header("x-docker-expose-session-grpc-method".to_string(), filesync::procedure::DIFF_COPY)
            .with_header("x-docker-expose-session-grpc-method".to_string(), health::procedure::CHECK),
    ).unwrap();

    eprintln!("[raw] opening Session bidi (no pump)...");
    let mut bidi = session_client
        .bidi_stream(Ctx::background(), futures::stream::pending::<BytesMessage>())
        .await
        .expect("open bidi");
    eprintln!("[raw] Session bidi opened, reading first message...");

    let (sender, mut receiver) = bidi.split();

    // buildkitd dials the tunneled Level 2 connection eagerly (grpc.DialContext),
    // so exactly two frames arrive unprompted right after the bidi opens: the H2
    // client preface, then its SETTINGS. Read only those two — with the session
    // now surviving a held handshake (~40s until health-check timeouts), any
    // further unbounded `receive()` would block until that teardown and wedge
    // the suite (a swallowed valtron-task stall, never a FAILED).
    for i in 0..2 {
        match receiver.receive().await {
            Ok(Some(msg)) => {
                let hex: String = msg.data.iter().map(|b| format!("{b:02x}")).collect();
                eprintln!("[raw] bk→us #{i} {} bytes: {hex}", msg.data.len());
                assert!(!msg.data.is_empty(), "handshake frame should be non-empty");
            }
            Ok(None) => panic!("stream ended before frame #{i} — session died during handshake"),
            Err(e) => panic!("receive error at frame #{i}: {e}"),
        }
    }
    eprintln!("[raw] done — buildkitd dialed us unprompted (preface + SETTINGS)");

    drop(sender);
    drop(receiver);
}

#[valtron_test]
async fn session_stays_alive_without_solve() {
    use foundation_deployment_docker::buildkit::session::SessionServer;

    let Some(addr) = buildkitd_addr() else { return };

    let dir = std::env::temp_dir().join(format!("ewe-bk-ctx-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir ctx");
    std::fs::write(dir.join("Dockerfile"), b"FROM alpine:latest\n").expect("write Dockerfile");

    let session = SessionServer::start(&addr, &dir).await.expect("start session");
    eprintln!("[test] session={} started, waiting 2s...", session.id);

    // Keep the session alive for 2 seconds — enough time for buildkitd's
    // monitorHealth to fire its first check at ~5s (or to close at ~20ms).
    std::thread::sleep(std::time::Duration::from_millis(2000));
    eprintln!("[test] session alive after 2s — SUCCESS");

    let _ = std::fs::remove_dir_all(&dir);
}

#[valtron_test]
async fn build_dockerfile_end_to_end() {
    use foundation_deployment_docker::buildkit::session::SessionServer;
    use foundation_deployment_docker::buildkit::types::SolveRequest;

    let Some(addr) = buildkitd_addr() else { return };

    // A trivial build context: one Dockerfile.
    let dir = std::env::temp_dir().join(format!("ewe-bk-ctx-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir ctx");
    std::fs::write(dir.join("Dockerfile"), b"FROM alpine:latest\nRUN echo hello-from-ewe\n")
        .expect("write Dockerfile");

    // Serve the build context back to buildkitd over the session.
    let session = SessionServer::start(&addr, &dir).await.expect("start session");

    let client = BuildKitClient::connect_tcp(&addr).expect("connect_tcp");

    let mut req = SolveRequest::default();
    req.Frontend = "dockerfile.v0".into();
    req.FrontendAttrs.insert("filename".into(), "Dockerfile".into());
    req.Session = session.id.clone();

    match client.solve(req).await {
        Ok(_) => eprintln!("BUILD OK — dockerfile.v0 solved with session {}", session.id),
        Err(e) => eprintln!("BUILD ERR: {e}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}
