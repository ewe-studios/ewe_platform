//! Docker-backed integration tests for `Runner` execution strategies.
//!
//! **WHY:** `Runner` fans a command across many hosts. The strategy shape tests
//! (Docker-free) stay in `foundation_sshkit`. The real-container test lives here
//! in `foundation_deployment_platform` — the orchestration layer that owns
//! container lifecycle.
//!
//! **WHAT:** Runs Parallel, Sequential, and Group strategies against a real sshd
//! container, asserting every host reports success.
//!
//! **HOW:** The test is `#[ignore]` (needs Docker). Blocking ssh2 fan-out runs on
//! `spawn_blocking` while the container lifecycle stays on the shared tokio
//! runtime.

// `foundation_sshkit` is only linked with the `vms` feature; gate the file so it
// does not break `cargo test` under the crate's default features.
#![cfg(feature = "vms")]

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use foundation_deployment_platform::docker::{ContainerConfig, ContainerHandle, WaitFor};
use foundation_sshkit::{Command, ConnectionPool, Host, Runner, Ssh2Backend};

static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
});

const SSH_USER: &str = "testuser";
const SSH_PASSWORD: &str = "testpass";
const SSH_PORT: u16 = 2222;

fn docker_available() -> bool {
    std::path::Path::new("/var/run/docker.sock").exists()
}

fn sshd_config() -> ContainerConfig {
    ContainerConfig::new("lscr.io/linuxserver/openssh-server:latest")
        .port(SSH_PORT)
        .env("PUID", "1000")
        .env("PGID", "1000")
        .env("TZ", "Etc/UTC")
        .env("PASSWORD_ACCESS", "true")
        .env("USER_NAME", SSH_USER)
        .env("USER_PASSWORD", SSH_PASSWORD)
        .wait(WaitFor::all(vec![
            WaitFor::port_with_timeout(SSH_PORT, Duration::from_secs(60)),
            WaitFor::stdout("[ls.io-init] done."),
        ]))
        .stop_timeout_secs(3)
}

#[test]
#[ignore = "requires Docker daemon"]
fn test_all_strategies_against_real_container() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }
    RT.block_on(async {
        let handle = ContainerHandle::start_async(sshd_config())
            .await
            .expect("start sshd container");
        let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

        let all_ok = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let pool = Arc::new(ConnectionPool::new(Duration::from_secs(30)));
            let backend = Ssh2Backend::new(pool);
            let hosts: Vec<Host> = (0..3)
                .map(|_| {
                    Host::parse(&format!("{SSH_USER}@127.0.0.1:{port}"))
                        .with_password(SSH_PASSWORD)
                })
                .collect();

            let builder = |_h: &Host| Command::new("echo runner-ok");

            for runner in [
                Runner::parallel(),
                Runner::sequential(Duration::from_millis(5)),
                Runner::group(2),
            ] {
                let results = runner.run(&hosts, &backend, builder);
                if results.len() != hosts.len() {
                    return Err(format!(
                        "expected {} results, got {}",
                        hosts.len(),
                        results.len()
                    ));
                }
                for r in results {
                    let cr = r.map_err(|e| format!("host execute failed: {e}"))?;
                    if cr.exit_code != 0 || !cr.stdout.contains("runner-ok") {
                        return Err(format!("unexpected result: {cr:?}"));
                    }
                }
            }
            Ok(())
        })
        .await
        .expect("join blocking runner task");

        all_ok.expect("all strategies should succeed against the container");
    });
}
