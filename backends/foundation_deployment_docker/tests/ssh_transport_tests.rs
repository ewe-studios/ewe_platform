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

/// `user@host[:port]` or `None` to skip (no SSH docker host configured/reachable).
fn ssh_target() -> Option<String> {
    let spec = std::env::var("EWE_DOCKER_SSH_HOST").ok()?;
    // Reachability probe: parse host:port (default 22) and try a TCP connect.
    let after_user = spec.rsplit('@').next().unwrap_or(&spec);
    let hostport = if after_user.contains(':') {
        after_user.to_string()
    } else {
        format!("{after_user}:22")
    };
    if std::net::TcpStream::connect(&hostport).is_err() {
        eprintln!("skipping: SSH host {hostport} unreachable");
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
