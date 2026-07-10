//! Integration tests for `RusshBackend` — Docker-free path.
//!
//! **WHY:** `russh.rs` is gated behind the `russh-backend` feature.  Docker-backed
//! tests live in `foundation_deployment_platform/tests/russh_backend_tests.rs`.
//! This file covers the connection-failure path, which needs no container.
//!
//! **WHAT:** Connection to a closed port must return a connect error.
//!
//! **HOW:** The whole file is gated on `russh-backend`.  A keypair is generated
//! with the system `ssh-keygen`; the test skips cleanly if it is unavailable.
#![cfg(feature = "russh-backend")]

use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

use foundation_sshkit::{Command, Host, RusshBackend};

static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
});

const SSH_USER: &str = "testuser";

struct KeyPair {
    private_path: PathBuf,
    #[allow(dead_code)]
    public_text: String,
}

fn generate_keypair() -> Option<KeyPair> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("sshkit_russh_{stamp}"));
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

fn russh_host(port: u16, key: &KeyPair) -> Host {
    Host::parse(&format!("{SSH_USER}@127.0.0.1:{port}")).with_key(key.private_path.clone())
}

#[test]
fn test_russh_execute_connection_refused() {
    let Some(key) = generate_keypair() else {
        tracing::warn!("SKIP: ssh-keygen not available");
        return;
    };

    RT.block_on(async {
        let backend = RusshBackend::default();
        let host = russh_host(1, &key);
        let result = backend.execute_async(&host, &Command::new("true")).await;

        assert!(result.is_err(), "connecting to a closed port must fail");
        let err = result.unwrap_err();
        assert!(
            err.contains("connect"),
            "error should originate from the connect step, got: {err:?}"
        );
    });
}
