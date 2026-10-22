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
//! **HOW:** The test is `#[ignore]` (needs Docker). It is a `#[valtron_test]`,
//! which keeps a valtron pool alive for the Docker client. The test thread is
//! the blocking boundary: container lifecycle uses the blocking
//! `ContainerHandle` wrappers, and the ssh2 fan-out (blocking FFI) runs directly
//! on the test thread, so no pool worker is held by it.

// `foundation_sshkit` is only linked with the `vms` feature; gate the file so it
// does not break `cargo test` under the crate's default features.
#![cfg(feature = "vms")]

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::valtron_test;
use foundation_deployment_platform::docker::{ContainerConfig, ContainerHandle, WaitFor};
use foundation_sshkit::{Command, ConnectionPool, Host, Runner, Ssh2Backend};

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

/// Run every `Runner` strategy against three hosts that all point at the
/// container, and check that each host reports success. Blocking libssh2 calls
/// run here, on the test thread, not on a valtron pool worker.
fn run_all_strategies(port: u16) -> Result<(), String> {
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(30)));
    let backend = Ssh2Backend::new(pool);
    let hosts: Vec<Host> = (0..3)
        .map(|_| Host::parse(&format!("{SSH_USER}@127.0.0.1:{port}")).with_password(SSH_PASSWORD))
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
}

#[valtron_test(seed = 54, threads = 4)]
#[ignore = "requires Docker daemon"]
fn test_all_strategies_against_real_container() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }
    let handle = ContainerHandle::start(sshd_config()).expect("start sshd container");
    let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

    let all_ok = run_all_strategies(port);

    all_ok.expect("all strategies should succeed against the container");
}
