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

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::buildkit::BuildKitClient;

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
