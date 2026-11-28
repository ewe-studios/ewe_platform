//! The `HashiCorp` Vault Transit backend, against a Vault dev server.
//!
//! WHY: no local stub proves the Transit protocol, the token header and the
//! derived-key encryption. This test uses the real Transit engine.
//!
//! WHAT: seal a payload, open it back and refuse a different record.
//!
//! HOW: the test uses the server named by `VAULT_ADDR` (the compose file
//! `ci/local-services/docker-compose.yml` starts it) when that variable is
//! set. Without it the test starts the same pinned image in a container and
//! removes the container on drop. The test prints the reason and returns when
//! neither form is available.

use std::process::Command;
use std::time::Duration;

use foundation_auth::keystore::{EnvelopeSealer, KeySealer, VaultTransitBackend};
use foundation_compact::crypto::SecureBytes;
use foundation_core::valtron::valtron_test;
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};

use super::common::test_ctx;

/// The image of the local-services compose file, pinned by digest so a CI run
/// and a laptop run use the same bytes.
const IMAGE: &str =
    "hashicorp/vault@sha256:dfa48c2fa2b0099ccb47681ec5c4804bf5aec9d6879ecfee07ad4b92a5427ad6";
const KEY_NAME: &str = "ewe-test";
const ROOT_TOKEN: &str = "root";
/// The longest wait for a Vault dev server to answer its health check.
const READY_TIMEOUT: Duration = Duration::from_secs(60);

/// A running Vault server. Drop removes a container that this test started.
struct Vault {
    /// The container name, when this test started the container.
    name: Option<String>,
    address: String,
    token: String,
    http: DynNetClient,
}

impl Drop for Vault {
    fn drop(&mut self) {
        if let Some(name) = &self.name {
            let _ = Command::new("docker")
                .args(["rm", "-f", name.as_str()])
                .output();
        }
    }
}

fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// A free loopback port.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
    listener.local_addr().expect("the local address").port()
}

/// Send a Vault request and return its status and body.
async fn vault_request(
    vault: &Vault,
    method: &str,
    url: &str,
    body: Option<&str>,
) -> Result<(usize, String), String> {
    let builder = match method {
        "GET" => PreparedRequestBuilder::get(url),
        "POST" => PreparedRequestBuilder::post(url),
        other => return Err(format!("unsupported method {other}")),
    }
    .map_err(|e| e.to_string())?
    .header("X-Vault-Token".to_string(), vault.token.clone());
    let builder = match body {
        Some(body) => builder.body_text(body),
        None => builder,
    };
    let response = vault
        .http
        .send_async(builder.build())
        .await
        .map_err(|e| e.to_string())?;
    let status: usize = response.get_status().into();
    let text = foundation_auth::shared::body::read_body_text(response.take_async_body()).await?;
    Ok((status, text))
}

/// Wait for the dev server to answer its health check.
async fn wait_ready(vault: &Vault) -> bool {
    let clock = foundation_context::Ctx::host().clock();
    let deadline = clock.now() + READY_TIMEOUT;
    loop {
        if let Ok((status, _)) = vault_request(
            vault,
            "GET",
            &format!("{}/v1/sys/health", vault.address),
            None,
        )
        .await
        {
            if status == 200 {
                return true;
            }
        }
        if clock.now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Enable the Transit secrets engine. A dev server starts with none.
async fn enable_transit(vault: &Vault) -> Result<(), String> {
    let (status, body) = vault_request(
        vault,
        "POST",
        &format!("{}/v1/sys/mounts/transit", vault.address),
        Some("{\"type\": \"transit\"}"),
    )
    .await?;
    // 400 means that the path is already mounted, which the compose file's
    // `vault-init` service does.
    if (200..=299).contains(&status) || status == 400 {
        Ok(())
    } else {
        Err(format!("enabling transit failed: {status} {body}"))
    }
}

/// Create the derived Transit key that the sealer uses.
async fn create_key(vault: &Vault) -> Result<(), String> {
    let (status, body) = vault_request(
        vault,
        "POST",
        &format!("{}/v1/transit/keys/{KEY_NAME}", vault.address),
        Some("{\"derived\": true}"),
    )
    .await?;
    if (200..=299).contains(&status) {
        Ok(())
    } else {
        Err(format!("creating the key failed: {status} {body}"))
    }
}

/// Start a Vault dev server. Returns `None` when Docker is absent or the image
/// cannot run.
async fn start_container() -> Option<Vault> {
    if !docker_available() {
        eprintln!("skipping the Vault test: Docker is not available");
        return None;
    }
    let port = free_port();
    let name = format!("ewe-vault-test-{}", std::process::id());
    let output = Command::new("docker")
        .args([
            "run",
            "-d",
            "--rm",
            "--name",
            &name,
            "-p",
            &format!("127.0.0.1:{port}:8200"),
            "-e",
            &format!("VAULT_DEV_ROOT_TOKEN_ID={ROOT_TOKEN}"),
            "-e",
            "VAULT_DEV_LISTEN_ADDRESS=0.0.0.0:8200",
            IMAGE,
        ])
        .output()
        .expect("run docker");
    if !output.status.success() {
        eprintln!(
            "skipping the Vault test: docker run failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }
    let vault = Vault {
        name: Some(name),
        address: format!("http://127.0.0.1:{port}"),
        token: ROOT_TOKEN.to_string(),
        http: HttpClientBuilder::new().build(),
    };
    if !wait_ready(&vault).await {
        eprintln!("skipping the Vault test: the dev server did not become healthy");
        return None;
    }
    if let Err(reason) = enable_transit(&vault).await {
        eprintln!("skipping the Vault test: {reason}");
        return None;
    }
    if let Err(reason) = create_key(&vault).await {
        eprintln!("skipping the Vault test: {reason}");
        return None;
    }
    Some(vault)
}

/// The server under test: the address from the environment, else a container.
async fn start_vault() -> Option<Vault> {
    let address = std::env::var("VAULT_ADDR")
        .ok()
        .filter(|address| !address.trim().is_empty());
    let Some(address) = address else {
        return start_container().await;
    };
    let vault = Vault {
        name: None,
        address,
        token: std::env::var("VAULT_TOKEN").unwrap_or_else(|_| ROOT_TOKEN.to_string()),
        http: HttpClientBuilder::new().build(),
    };
    if !wait_ready(&vault).await {
        eprintln!(
            "skipping the Vault test: {} did not become healthy",
            vault.address
        );
        return None;
    }
    if let Err(reason) = enable_transit(&vault).await {
        eprintln!("skipping the Vault test: {reason}");
        return None;
    }
    if let Err(reason) = create_key(&vault).await {
        eprintln!("skipping the Vault test: {reason}");
        return None;
    }
    Some(vault)
}

#[valtron_test(threads = 4, timeout = 600_000)]
async fn vault_transit_seals_and_opens() {
    let Some(vault) = start_vault().await else {
        return;
    };
    let (_ctx, env) = test_ctx();
    let backend = VaultTransitBackend::new(
        vault.address.clone(),
        KEY_NAME.to_string(),
        SecureBytes::new(vault.token.as_bytes().to_vec()),
        vault.http.clone(),
    );
    let cipher = EnvelopeSealer::new("vault-transit", std::sync::Arc::new(backend), env);

    let aad = b"record:ca:1";
    let secret = b"a private key in PKCS#8 DER";
    let sealed = cipher.seal(aad, secret).await.expect("seal");
    assert_eq!(sealed.alg, "kms-envelope+chacha20poly1305");
    let opened = cipher.open(aad, &sealed).await.expect("open");
    assert_eq!(opened.as_slice(), secret);

    // A different record cannot open the blob.
    assert!(cipher.open(b"record:ca:2", &sealed).await.is_err());
}
