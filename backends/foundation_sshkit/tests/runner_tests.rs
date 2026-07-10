//! Tests for `Runner` execution strategies.
//!
//! **WHY:** `Runner` fans a command out across many hosts using one of three
//! strategies (Parallel, Sequential, Group). The strategy code paths — result
//! ordering, batching, and per-host command building — were previously
//! untested.
//!
//! **WHAT:** Two layers of coverage:
//!   1. Docker-free tests point every strategy at unreachable hosts and assert
//!      the shape of the returned `Vec` (one result per host, all errors). This
//!      exercises every match arm without needing a server.
//!   2. A Docker-backed test (`#[ignore]`) runs all three strategies against a
//!      real sshd container and asserts every host reports success.
//!
//! **HOW:** The unreachable-host tests are pure and synchronous. The container
//! test runs the blocking ssh2 fan-out on `spawn_blocking` while the container
//! lifecycle stays on a shared multi-threaded tokio runtime.

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

/// Hosts that will never accept a connection (loopback low ports refuse TCP).
fn unreachable_hosts(count: usize) -> Vec<Host> {
    (0..count)
        .map(|i| Host::parse(&format!("root@127.0.0.1:{}", i + 1)))
        .collect()
}

// ── Docker-free strategy shape tests ──

#[test]
fn test_parallel_returns_one_result_per_host() {
    let hosts = unreachable_hosts(3);
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(1)));
    let backend = Ssh2Backend::new(pool);

    let results = Runner::parallel().run(&hosts, &backend, |_h| Command::new("true"));

    assert_eq!(results.len(), hosts.len(), "one result per host");
    assert!(
        results.iter().all(Result::is_err),
        "every unreachable host should yield a connect error"
    );
}

#[test]
fn test_sequential_returns_one_result_per_host() {
    let hosts = unreachable_hosts(3);
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(1)));
    let backend = Ssh2Backend::new(pool);

    let results = Runner::sequential(Duration::from_millis(1))
        .run(&hosts, &backend, |_h| Command::new("true"));

    assert_eq!(results.len(), hosts.len(), "one result per host, in order");
    assert!(results.iter().all(Result::is_err));
}

#[test]
fn test_group_batches_cover_all_hosts() {
    // 5 hosts, batches of 2 → 3 chunks (2 + 2 + 1); every host still runs once.
    let hosts = unreachable_hosts(5);
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(1)));
    let backend = Ssh2Backend::new(pool);

    let results = Runner::group(2).run(&hosts, &backend, |_h| Command::new("true"));

    assert_eq!(results.len(), hosts.len(), "grouping must not drop hosts");
    assert!(results.iter().all(Result::is_err));
}

#[test]
fn test_command_builder_receives_each_host() {
    // Prove the closure is invoked per host by capturing the hostnames it sees.
    let hosts = vec![Host::parse("root@127.0.0.1:1"), Host::parse("root@127.0.0.1:2")];
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(1)));
    let backend = Ssh2Backend::new(pool);
    let seen = std::sync::Mutex::new(Vec::new());

    let results = Runner::parallel().run(&hosts, &backend, |h| {
        seen.lock().unwrap().push(h.port);
        Command::new("true")
    });

    assert_eq!(results.len(), 2);
    assert_eq!(*seen.lock().unwrap(), vec![1u16, 2u16], "builder saw both hosts");
}

// ── Docker-backed strategy tests ──

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
            // Three logical hosts, all pointing at the one container.
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
                    return Err(format!("expected {} results, got {}", hosts.len(), results.len()));
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
