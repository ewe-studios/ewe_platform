//! Integration test for the SSH transport (`ssh://user@host`).
//!
//! Exercises `DockerClient::connect_ssh` against a daemon reached over SSH +
//! `docker system dial-stdio`. Any host running sshd + docker where the test
//! user can run `docker system dial-stdio` works; docker-in-docker with sshd is
//! the easiest:
//!
//!   docker run -d --privileged --name ewe-dind-tls docker:dind
//!   docker exec ewe-dind-tls sh -c 'apk add openssh && ssh-keygen -A && \
//!     mkdir -p /root/.ssh && echo "<pubkey>" > /root/.ssh/authorized_keys && \
//!     /usr/sbin/sshd'
//!
//! then run with the key loaded into an ssh-agent (sshkit falls back to the
//! agent when the `ssh://` URL carries no key) and:
//!   EWE_DOCKER_SSH_HOST=root@<container-ip>
//!
//! Skips cleanly when the host is unset or unreachable.

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

/// `user@host[:port]` (or a `~/.ssh/config` alias) or `None` to skip.
///
/// Reachability is probed with a TCP connect. For a bare `host`/`user@host` the
/// probe target is derived from the spec; for a config **alias** (which the host
/// machine can't resolve) set `EWE_DOCKER_SSH_PROBE=ip:port` to probe instead.
fn ssh_target() -> Option<String> {
    let spec = std::env::var("EWE_DOCKER_SSH_HOST").ok()?;
    let probe = std::env::var("EWE_DOCKER_SSH_PROBE").unwrap_or_else(|_| {
        let after_user = spec.rsplit('@').next().unwrap_or(&spec);
        if after_user.contains(':') {
            after_user.to_string()
        } else {
            format!("{after_user}:22")
        }
    });
    if std::net::TcpStream::connect(&probe).is_err() {
        eprintln!("skipping: SSH host probe {probe} unreachable");
        return None;
    }
    Some(spec)
}

#[valtron_test]
async fn info_over_ssh() {
    let Some(spec) = ssh_target() else { return };
    let url = format!("ssh://{spec}");
    let client = DockerClient::connect_ssh(&url).expect("connect_ssh");
    assert!(
        client.base_url().starts_with("http://localhost"),
        "SSH base_url must be http://localhost (dial-stdio bridges to remote sock)"
    );

    let info = client.system_info().await.expect("Info over SSH");
    assert!(!info.is_null(), "SystemInfo should be valid JSON");
    eprintln!("SSH INFO OK — {}", client.base_url());
}

#[valtron_test]
async fn container_round_trip_over_ssh() {
    let Some(spec) = ssh_target() else { return };
    let c = DockerClient::connect_ssh(&format!("ssh://{spec}")).expect("connect_ssh");

    let _ = c.remove_container("ewe-ssh-rt", true).await;
    let id = c
        .create_container(
            &serde_json::json!({"Image": "alpine:latest", "Cmd": ["sleep", "30"]}),
            Some("ewe-ssh-rt"),
        )
        .await
        .expect("create over SSH");
    c.start_container(&id.id).await.expect("start over SSH");
    let inspect = c.inspect_container(&id.id).await.expect("inspect over SSH");
    assert!(
        inspect.state.and_then(|s| s.running).unwrap_or(false),
        "container should be running"
    );
    c.remove_container(&id.id, true).await.expect("remove over SSH");
    eprintln!("SSH CONTAINER ROUND-TRIP OK");
}
