//! Integration tests for `Ssh2Backend` against a real sshd container.
//!
//! **WHY:** The ssh2 backend is the primary SSH transport. Unit-testing the
//! command/host builders proves the wiring but not that `execute`, `upload`,
//! and `download` actually talk to an SSH server. These tests stand up a real
//! `linuxserver/openssh-server` container via the spec-53 docker testbed
//! (`foundation_deployment_platform`) and drive the backend end-to-end.
//!
//! **WHAT:** Covers `execute` (success, non-zero exit, stdout+stderr capture),
//! `upload` + `download` round-trip, and the connection/auth failure paths.
//!
//! **HOW:** Container-backed tests are `#[ignore]` (they need a Docker daemon)
//! and are driven from a shared multi-threaded tokio runtime — the ssh2 calls
//! are blocking libssh2 FFI, so they run on `spawn_blocking` while the container
//! lifecycle stays async. The connection-refused test needs no Docker and runs
//! in the default suite.

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use foundation_deployment_platform::docker::{ContainerConfig, ContainerHandle, WaitFor};
use foundation_sshkit::{Command, ConnectionPool, Host, Ssh2Backend};
use foundation_sshkit::Backend;

/// Shared multi-threaded runtime so container-lifecycle futures make progress
/// while blocking ssh2 work runs on `spawn_blocking` worker threads.
static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
});

/// The in-container username/password the openssh image is configured with.
const SSH_USER: &str = "testuser";
const SSH_PASSWORD: &str = "testpass";
const SSH_PORT: u16 = 2222;

fn docker_available() -> bool {
    std::path::Path::new("/var/run/docker.sock").exists()
}

/// Build a password-auth sshd container config. Readiness = TCP port open AND
/// the linuxserver init reporting `done.` (sshd fully up).
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

/// A backend + host pointing at the container's mapped ssh port.
fn backend_and_host(host_port: u16) -> (Ssh2Backend, Host) {
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(30)));
    let backend = Ssh2Backend::new(pool);
    let host = Host::parse(&format!("{SSH_USER}@127.0.0.1:{host_port}"))
        .with_password(SSH_PASSWORD);
    (backend, host)
}

#[test]
#[ignore = "requires Docker daemon"]
fn test_execute_success_captures_stdout() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }

    RT.block_on(async {
        let handle = ContainerHandle::start_async(sshd_config())
            .await
            .expect("start sshd container");
        let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

        let result = tokio::task::spawn_blocking(move || {
            let (backend, host) = backend_and_host(port);
            backend.execute(&host, &Command::new("echo hello-sshkit"))
        })
        .await
        .expect("join blocking ssh task")
        .expect("execute should succeed");

        assert_eq!(result.exit_code, 0, "echo should exit 0");
        assert!(
            result.stdout.contains("hello-sshkit"),
            "stdout should contain the echoed text, got: {:?}",
            result.stdout
        );
        assert!(result.host.contains(SSH_USER), "host label should carry the user");
    });
}

#[test]
#[ignore = "requires Docker daemon"]
fn test_execute_nonzero_exit_code() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }

    RT.block_on(async {
        let handle = ContainerHandle::start_async(sshd_config())
            .await
            .expect("start sshd container");
        let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

        let result = tokio::task::spawn_blocking(move || {
            let (backend, host) = backend_and_host(port);
            // Rendered as a single command string and run via the remote shell.
            backend.execute(&host, &Command::new("exit 7"))
        })
        .await
        .expect("join blocking ssh task")
        .expect("execute should still return Ok — the command ran, it just failed");

        assert_eq!(result.exit_code, 7, "remote `exit 7` should surface as exit code 7");
        assert!(!result.is_success(), "non-zero exit is not a success");
    });
}

#[test]
#[ignore = "requires Docker daemon"]
fn test_execute_captures_stdout_and_stderr() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }

    RT.block_on(async {
        let handle = ContainerHandle::start_async(sshd_config())
            .await
            .expect("start sshd container");
        let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

        let result = tokio::task::spawn_blocking(move || {
            let (backend, host) = backend_and_host(port);
            backend.execute(&host, &Command::new("echo out-line; echo err-line 1>&2"))
        })
        .await
        .expect("join blocking ssh task")
        .expect("execute should succeed");

        assert_eq!(result.exit_code, 0);
        assert!(
            result.stdout.contains("out-line"),
            "stdout should hold the stdout write, got: {:?}",
            result.stdout
        );
        assert!(
            result.stderr.contains("err-line"),
            "stderr should hold the stderr write, got: {:?}",
            result.stderr
        );
    });
}

#[test]
#[ignore = "requires Docker daemon"]
fn test_upload_then_download_round_trips_a_file() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }

    RT.block_on(async {
        let handle = ContainerHandle::start_async(sshd_config())
            .await
            .expect("start sshd container");
        let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

        let outcome = tokio::task::spawn_blocking(move || -> Result<(String, String), String> {
            let (backend, host) = backend_and_host(port);

            // Unique local paths so parallel test runs don't collide.
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir();
            let src = dir.join(format!("sshkit_up_{stamp}.txt"));
            let dst = dir.join(format!("sshkit_down_{stamp}.txt"));
            let payload = format!("round-trip payload {stamp}\nsecond line\n");

            std::fs::write(&src, &payload).map_err(|e| format!("write src: {e}"))?;

            // testuser owns /config in the linuxserver image — a writable target.
            let remote = std::path::PathBuf::from("/config/sshkit_roundtrip.txt");
            backend.upload(&host, &src, &remote)?;

            // Confirm the bytes actually landed on the server via a remote read.
            let cat = backend.execute(&host, &Command::new("cat /config/sshkit_roundtrip.txt"))?;

            backend.download(&host, &remote, &dst)?;
            let downloaded = std::fs::read_to_string(&dst).map_err(|e| format!("read dst: {e}"))?;

            std::fs::remove_file(&src).ok();
            std::fs::remove_file(&dst).ok();

            Ok((cat.stdout, downloaded))
        })
        .await
        .expect("join blocking ssh task");

        let (remote_cat, downloaded) = outcome.expect("upload/download round-trip");
        assert!(
            remote_cat.contains("round-trip payload"),
            "remote file should contain the uploaded payload, got: {remote_cat:?}"
        );
        assert!(
            downloaded.contains("round-trip payload") && downloaded.contains("second line"),
            "downloaded file should match what was uploaded, got: {downloaded:?}"
        );
    });
}

#[test]
#[ignore = "requires Docker daemon"]
fn test_execute_wrong_password_is_rejected() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }

    RT.block_on(async {
        let handle = ContainerHandle::start_async(sshd_config())
            .await
            .expect("start sshd container");
        let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

        let result = tokio::task::spawn_blocking(move || {
            let pool = Arc::new(ConnectionPool::new(Duration::from_secs(10)));
            let backend = Ssh2Backend::new(pool);
            let host = Host::parse(&format!("{SSH_USER}@127.0.0.1:{port}"))
                .with_password("definitely-the-wrong-password");
            backend.execute(&host, &Command::new("whoami"))
        })
        .await
        .expect("join blocking ssh task");

        assert!(
            result.is_err(),
            "authentication with a wrong password must fail, got: {result:?}"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("auth"),
            "error should point at the auth step, got: {err:?}"
        );
    });
}

#[test]
fn test_execute_connection_refused_returns_error() {
    // No Docker needed: port 1 on loopback refuses TCP, so the pool cannot
    // even open a session. This exercises the connect-failure path of execute.
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(2)));
    let backend = Ssh2Backend::new(pool);
    let host = Host::parse("root@127.0.0.1:1");

    let result = backend.execute(&host, &Command::new("true"));

    assert!(result.is_err(), "connecting to a closed port must fail");
    let err = result.unwrap_err();
    assert!(
        err.contains("connect"),
        "error should originate from the TCP connect, got: {err:?}"
    );
}
