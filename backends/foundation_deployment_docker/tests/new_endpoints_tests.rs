//! Integration tests for the container/image endpoints added to reach full
//! bollard-0.21 HTTP parity: `update_container`, `upload_to_container`,
//! `get_container_archive_info`, and `export_images`.
//!
//! These run against a real dockerd over the Unix socket.
//!
//! Run: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat
//!   --test new_endpoints_tests -- --test-threads=1`

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

fn client() -> DockerClient {
    DockerClient::connect_unix("/var/run/docker.sock")
}

/// Create + start a long-lived alpine container, returning its id.
async fn start_alpine(c: &DockerClient, name: &str) -> String {
    let _ = std::process::Command::new("docker")
        .args(["rm", "-f", name])
        .output();
    let id = c
        .create_container(
            &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
            Some(name),
        )
        .await
        .expect("create");
    c.start_container(&id.id).await.expect("start");
    id.id
}

// ── upload_to_container round-trips a Docker-produced tar ────────────────

#[valtron_test]
async fn upload_and_download_archive_round_trips() {
    let c = client();
    let id = start_alpine(&c, "ewe-it-upload").await;

    // Download an existing file as a valid tar straight from Docker, then
    // upload that exact tar into /tmp and read it back — avoids constructing
    // tar bytes by hand while exercising both PUT and GET archive paths.
    let tar = c
        .container_archive(&id, "/etc/hostname")
        .await
        .expect("download /etc/hostname");
    assert!(!tar.is_empty(), "downloaded tar should be non-empty");

    c.upload_to_container(&id, "/tmp", tar.clone(), Some(false), Some(false))
        .await
        .expect("upload tar into /tmp");

    let back = c
        .container_archive(&id, "/tmp/hostname")
        .await
        .expect("download /tmp/hostname after upload");
    assert!(!back.is_empty(), "re-downloaded tar should be non-empty");

    let _ = c.remove_container(&id, true).await;
}

// ── get_container_archive_info returns a stat header ─────────────────────

#[valtron_test]
async fn get_container_archive_info_ok() {
    let c = client();
    let id = start_alpine(&c, "ewe-it-archive-info").await;

    let stat = c
        .get_container_archive_info(&id, "/etc/hostname")
        .await
        .expect("archive info for /etc/hostname");
    // Docker returns a base64-encoded JSON FileInfo blob.
    assert!(!stat.is_empty(), "stat header should be non-empty");

    // A missing path must error, not silently succeed.
    let missing = c
        .get_container_archive_info(&id, "/no/such/path")
        .await;
    assert!(missing.is_err(), "missing path should error");

    let _ = c.remove_container(&id, true).await;
}

// ── update_container adjusts resource limits ─────────────────────────────

#[valtron_test]
async fn update_container_cpu_shares() {
    let c = client();
    let id = start_alpine(&c, "ewe-it-update").await;

    // CpuShares is a safe, cgroup-independent knob to flip on any host.
    c.update_container(&id, serde_json::json!({"CpuShares": 512}))
        .await
        .expect("update container cpu shares");

    let _ = c.remove_container(&id, true).await;
}

// ── export_images streams a multi-image tar ──────────────────────────────

#[valtron_test]
async fn export_images_alpine() {
    let c = client();
    // alpine:latest is present — every other test pulls/uses it.
    let tar = c
        .export_images(&["alpine:latest"])
        .await
        .expect("export alpine:latest");
    assert!(
        tar.len() > 512,
        "exported image tar should be larger than one tar block"
    );
}
