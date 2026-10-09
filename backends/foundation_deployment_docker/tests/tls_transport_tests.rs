//! Integration test for the TLS (mutual-TLS) transport.
//!
//! Exercises `DockerClient::connect_tls` against a `--tlsverify` daemon. The
//! easiest such daemon is docker-in-docker with `DOCKER_TLS_CERTDIR`, which
//! auto-generates a CA + server + client certs:
//!
//!   docker run -d --privileged --name ewe-dind-tls \
//!     -e DOCKER_TLS_CERTDIR=/certs -v /tmp/ewe-dind-certs:/certs \
//!     -p 12376:2376 docker:dind
//!
//! then `EWE_DOCKER_TLS_CERTS=/tmp/ewe-dind-certs/client` and
//! `EWE_DOCKER_TLS_HOST=127.0.0.1:12376` (both default to those values). Skips
//! cleanly when the certs or daemon are absent.

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::{DockerClient, DockerTls};

fn certs_dir() -> String {
    std::env::var("EWE_DOCKER_TLS_CERTS")
        .unwrap_or_else(|_| "/tmp/ewe-dind-certs/client".to_string())
}

fn tls_host() -> String {
    std::env::var("EWE_DOCKER_TLS_HOST").unwrap_or_else(|_| "127.0.0.1:12376".to_string())
}

/// `(host, DockerTls)` or `None` to skip (no TLS daemon / certs available).
fn tls_target(verify: bool) -> Option<(String, DockerTls)> {
    let host = tls_host();
    if std::net::TcpStream::connect(&host).is_err() {
        eprintln!("skipping: no TLS docker daemon at {host}");
        return None;
    }
    match DockerTls::from_cert_dir(certs_dir(), verify) {
        Ok(tls) => Some((host, tls)),
        Err(e) => {
            eprintln!("skipping: no client certs in {} ({e})", certs_dir());
            None
        }
    }
}

#[valtron_test]
async fn info_over_mutual_tls() {
    // verify = true: full mTLS — client cert presented AND daemon verified
    // against the dind CA. (dind's server cert carries 127.0.0.1 as a SAN.)
    let Some((host, tls)) = tls_target(true) else { return };
    let client = DockerClient::connect_tls(&host, tls).expect("connect_tls");
    assert!(client.base_url().starts_with("https://"), "TLS base_url must be https");

    let info = client.system_info().await.expect("Info over mTLS");
    assert!(!info.is_null(), "SystemInfo should be valid JSON");
    eprintln!("TLS INFO OK — {}", client.base_url());
}

#[valtron_test]
async fn container_round_trip_over_tls() {
    let Some((host, tls)) = tls_target(true) else { return };
    let c = DockerClient::connect_tls(&host, tls).expect("connect_tls");

    let _ = c.remove_container("ewe-tls-rt", true).await;
    let id = c
        .create_container(
            &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
            Some("ewe-tls-rt"),
        )
        .await
        .expect("create over TLS");
    c.start_container(&id.id).await.expect("start over TLS");
    let inspect = c.inspect_container(&id.id).await.expect("inspect over TLS");
    assert!(
        inspect.state.and_then(|s| s.running).unwrap_or(false),
        "container should be running"
    );
    c.remove_container(&id.id, true).await.expect("remove over TLS");
    eprintln!("TLS CONTAINER ROUND-TRIP OK");
}

#[valtron_test]
async fn info_over_tls_insecure() {
    // verify = false: encrypted but server not authenticated (DOCKER_TLS_VERIFY=0).
    let Some((host, tls)) = tls_target(false) else { return };
    let client = DockerClient::connect_tls(&host, tls).expect("connect_tls insecure");
    let info = client.system_info().await.expect("Info over insecure TLS");
    assert!(!info.is_null());
    eprintln!("TLS INSECURE INFO OK");
}
