//! Docker-backed integration tests for `Ssh2Backend`.
//!
//! **WHY:** The ssh2 backend is the primary SSH transport. These tests stand up a
//! real `linuxserver/openssh-server` container and drive the backend end-to-end.
//! They live in `foundation_deployment_platform` — the orchestration layer that
//! owns container lifecycle and consumes SSH — rather than in `foundation_sshkit`
//! (a leaf primitive). That keeps the dependency graph clean: primitives never
//! depend on their consumers, even in dev.
//!
//! **WHAT:** Covers `execute` (success, non-zero exit, stdout+stderr capture),
//! `upload` + `download` round-trip, and auth rejection.
//!
//! **HOW:** All tests are `#[ignore]` (need Docker). Each test is a
//! `#[valtron_test]`, which keeps a valtron pool alive for the Docker client.
//! The test thread is the blocking boundary: container lifecycle uses the
//! blocking `ContainerHandle` wrappers, and the libssh2 calls (blocking FFI) run
//! directly on the test thread, so no pool worker is held by them.

// `foundation_sshkit` (the `Ssh2Backend` under test) is only linked with the
// `vms` feature; without this gate the file fails to compile under the crate's
// default features and breaks `cargo test` for every consumer.
#![cfg(feature = "vms")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use foundation_core::valtron::valtron_test;
use foundation_deployment_platform::docker::{ContainerConfig, ContainerHandle, WaitFor};
use foundation_sshkit::Backend;
use foundation_sshkit::{Command, ConnectionPool, Host, Ssh2Backend};

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

fn backend_and_host(host_port: u16) -> (Ssh2Backend, Host) {
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(30)));
    let backend = Ssh2Backend::new(pool);
    let host =
        Host::parse(&format!("{SSH_USER}@127.0.0.1:{host_port}")).with_password(SSH_PASSWORD);
    (backend, host)
}

/// Upload a file, read it back remotely with `cat`, then download it again.
/// Returns the remote `cat` output and the downloaded text. Blocking libssh2
/// calls run here, on the test thread, not on a valtron pool worker.
fn upload_then_download(port: u16) -> Result<(String, String), String> {
    let (backend, host) = backend_and_host(port);

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("clock before UNIX epoch: {e}"))?
        .as_nanos();
    let dir = env::temp_dir();
    let src = dir.join(format!("sshkit_up_{stamp}.txt"));
    let dst = dir.join(format!("sshkit_down_{stamp}.txt"));
    let payload = format!("round-trip payload {stamp}\nsecond line\n");

    fs::write(&src, &payload).map_err(|e| format!("write src: {e}"))?;

    let remote = PathBuf::from("/config/sshkit_roundtrip.txt");
    backend.upload(&host, &src, &remote)?;

    let cat = backend.execute(&host, &Command::new("cat /config/sshkit_roundtrip.txt"))?;

    backend.download(&host, &remote, &dst)?;
    let downloaded = fs::read_to_string(&dst).map_err(|e| format!("read dst: {e}"))?;

    fs::remove_file(&src).ok();
    fs::remove_file(&dst).ok();

    Ok((cat.stdout, downloaded))
}

#[valtron_test(seed = 54, threads = 4)]
#[ignore = "requires Docker daemon"]
fn test_execute_success_captures_stdout() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }
    let handle = ContainerHandle::start(sshd_config()).expect("start sshd container");
    let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

    let (backend, host) = backend_and_host(port);
    let result = backend
        .execute(&host, &Command::new("echo hello-sshkit"))
        .expect("execute should succeed");

    assert_eq!(result.exit_code, 0, "echo should exit 0");
    assert!(
        result.stdout.contains("hello-sshkit"),
        "stdout should contain the echoed text, got: {:?}",
        result.stdout
    );
}

#[valtron_test(seed = 54, threads = 4)]
#[ignore = "requires Docker daemon"]
fn test_execute_nonzero_exit_code() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }
    let handle = ContainerHandle::start(sshd_config()).expect("start sshd container");
    let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

    let (backend, host) = backend_and_host(port);
    let result = backend
        .execute(&host, &Command::new("exit 7"))
        .expect("execute should still return Ok — the command ran, it just failed");

    assert_eq!(
        result.exit_code, 7,
        "remote `exit 7` should surface as exit code 7"
    );
    assert!(!result.is_success(), "non-zero exit is not a success");
}

#[valtron_test(seed = 54, threads = 4)]
#[ignore = "requires Docker daemon"]
fn test_execute_captures_stdout_and_stderr() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }
    let handle = ContainerHandle::start(sshd_config()).expect("start sshd container");
    let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

    let (backend, host) = backend_and_host(port);
    let result = backend
        .execute(&host, &Command::new("echo out-line; echo err-line 1>&2"))
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
}

#[valtron_test(seed = 54, threads = 4)]
#[ignore = "requires Docker daemon"]
fn test_upload_then_download_round_trips_a_file() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }
    let handle = ContainerHandle::start(sshd_config()).expect("start sshd container");
    let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

    let outcome = upload_then_download(port);

    let (remote_cat, downloaded) = outcome.expect("upload/download round-trip");
    assert!(
        remote_cat.contains("round-trip payload"),
        "remote file should contain the uploaded payload, got: {remote_cat:?}"
    );
    assert!(
        downloaded.contains("round-trip payload") && downloaded.contains("second line"),
        "downloaded file should match what was uploaded, got: {downloaded:?}"
    );
}

#[valtron_test(seed = 54, threads = 4)]
#[ignore = "requires Docker daemon"]
fn test_execute_wrong_password_is_rejected() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }
    let handle = ContainerHandle::start(sshd_config()).expect("start sshd container");
    let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

    let result = {
        let pool = Arc::new(ConnectionPool::new(Duration::from_secs(10)));
        let backend = Ssh2Backend::new(pool);
        let host = Host::parse(&format!("{SSH_USER}@127.0.0.1:{port}"))
            .with_password("definitely-the-wrong-password");
        backend.execute(&host, &Command::new("whoami"))
    };

    assert!(
        result.is_err(),
        "authentication with a wrong password must fail, got: {result:?}"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("auth"),
        "error should point at the auth step, got: {err:?}"
    );
}
