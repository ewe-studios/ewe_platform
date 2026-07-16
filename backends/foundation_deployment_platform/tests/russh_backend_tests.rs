//! Docker-backed integration tests for `RusshBackend` (pure-Rust async SSH backend).
//!
//! **WHY:** `russh.rs` is a full working backend gated behind `russh-backend`.
//! These tests live in `foundation_deployment_platform` — the orchestration
//! layer that owns container lifecycle — rather than in `foundation_sshkit`.
//!
//! **WHAT:** `execute_async` success and `upload_async`/`download_async` SFTP
//! round-trip against a real sshd container configured with a generated ed25519
//! key.
//!
//! **HOW:** A keypair is generated with the system `ssh-keygen`; the public key
//! goes into the container via `PUBLIC_KEY`, the private key drives russh. Tests
//! are `#[ignore]` (need Docker). russh is natively async so calls run directly
//! on the tokio runtime.

// `foundation_sshkit`'s russh backend is only linked with the `vms` feature;
// gate the file so it does not break `cargo test` under default features.
#![cfg(feature = "vms")]

use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

use foundation_deployment_platform::docker::{ContainerConfig, ContainerHandle, WaitFor};
use foundation_sshkit::{Command, Host, RusshBackend};

static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
});

const SSH_USER: &str = "testuser";
const SSH_PORT: u16 = 2222;

fn docker_available() -> bool {
    std::path::Path::new("/var/run/docker.sock").exists()
}

struct KeyPair {
    private_path: PathBuf,
    public_text: String,
}

fn generate_keypair() -> Option<KeyPair> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("deploy_russh_{stamp}"));
    std::fs::create_dir_all(&dir).ok()?;
    let private_path = dir.join("id_ed25519");

    let status = std::process::Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", "", "-q", "-f"])
        .arg(&private_path)
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }

    let public_text = std::fs::read_to_string(dir.join("id_ed25519.pub")).ok()?;
    Some(KeyPair {
        private_path,
        public_text: public_text.trim().to_string(),
    })
}

fn sshd_config_with_key(public_key: &str) -> ContainerConfig {
    ContainerConfig::new("lscr.io/linuxserver/openssh-server:latest")
        .port(SSH_PORT)
        .env("PUID", "1000")
        .env("PGID", "1000")
        .env("TZ", "Etc/UTC")
        .env("USER_NAME", SSH_USER)
        .env("PUBLIC_KEY", public_key)
        .wait(WaitFor::all(vec![
            WaitFor::port_with_timeout(SSH_PORT, Duration::from_secs(60)),
            WaitFor::stdout("[ls.io-init] done."),
        ]))
        .stop_timeout_secs(3)
}

fn russh_host(port: u16, key: &KeyPair) -> Host {
    Host::parse(&format!("{SSH_USER}@127.0.0.1:{port}")).with_key(key.private_path.clone())
}

#[test]
#[ignore = "requires Docker daemon"]
fn test_russh_execute_success() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }
    let Some(key) = generate_keypair() else {
        tracing::warn!("SKIP: ssh-keygen not available");
        return;
    };
    RT.block_on(async {
        let handle = ContainerHandle::start_async(sshd_config_with_key(&key.public_text))
            .await
            .expect("start sshd container");
        let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

        let backend = RusshBackend::default();
        let host = russh_host(port, &key);
        let result = backend
            .execute_async(&host, &Command::new("echo russh-hello"))
            .await
            .expect("russh execute should succeed");

        assert_eq!(result.exit_code, 0, "echo should exit 0");
        assert!(
            result.stdout.contains("russh-hello"),
            "stdout should carry the echoed text, got: {:?}",
            result.stdout
        );
    });
}

#[test]
#[ignore = "requires Docker daemon"]
fn test_russh_upload_download_round_trips() {
    if !docker_available() {
        tracing::warn!("SKIP: Docker not available");
        return;
    }
    let Some(key) = generate_keypair() else {
        tracing::warn!("SKIP: ssh-keygen not available");
        return;
    };
    RT.block_on(async {
        let handle = ContainerHandle::start_async(sshd_config_with_key(&key.public_text))
            .await
            .expect("start sshd container");
        let port = handle.host_port(SSH_PORT).expect("ssh port mapped");

        let backend = RusshBackend::default();
        let host = russh_host(port, &key);

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir();
        let src = dir.join(format!("russh_up_{stamp}.txt"));
        let dst = dir.join(format!("russh_down_{stamp}.txt"));
        let payload = format!("russh sftp payload {stamp}\n");
        std::fs::write(&src, &payload).expect("write src");

        let remote = PathBuf::from("russh_roundtrip.txt");
        backend
            .upload_async(&host, &src, &remote)
            .await
            .expect("russh upload");
        backend
            .download_async(&host, &remote, &dst)
            .await
            .expect("russh download");

        let downloaded = std::fs::read_to_string(&dst).expect("read dst");
        std::fs::remove_file(&src).ok();
        std::fs::remove_file(&dst).ok();

        assert_eq!(downloaded, payload, "SFTP round-trip must preserve bytes");
    });
}
