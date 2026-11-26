//! The `HashiCorp` Vault Transit backend, against a Vault dev server in a
//! container.
//!
//! The test prints the reason and returns when Docker is absent, or when the
//! image cannot be pulled. It uses the real Vault Transit engine, not a stub.

use std::process::Command;
use std::time::Duration;

use foundation_auth::keystore::{EnvelopeSealer, KeySealer, VaultTransitBackend};
use foundation_compact::crypto::SecureBytes;
use foundation_core::valtron::valtron_test;
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};

use super::common::test_ctx;

const IMAGE: &str = "hashicorp/vault:latest";
const KEY_NAME: &str = "ewe-test";
const ROOT_TOKEN: &str = "root";

/// A running Vault container. Drop removes it.
struct Vault {
    name: String,
    address: String,
    http: DynNetClient,
}

impl Drop for Vault {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.name])
            .output();
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
    http: &DynNetClient,
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
    .header("X-Vault-Token".to_string(), ROOT_TOKEN);
    let builder = match body {
        Some(body) => builder.body_text(body),
        None => builder,
    };
    let response = http
        .send_async(builder.build())
        .await
        .map_err(|e| e.to_string())?;
    let status: usize = response.get_status().into();
    let text = foundation_auth::shared::body::read_body_text(response.take_async_body()).await?;
    Ok((status, text))
}

/// Start a Vault dev server. Returns `None` when Docker is absent or the image
/// cannot run.
async fn start_vault() -> Option<Vault> {
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

    let http = HttpClientBuilder::new().build();
    let address = format!("http://127.0.0.1:{port}");
    let vault = Vault {
        name,
        address: address.clone(),
        http: http.clone(),
    };

    // Wait for the dev server to answer its health check.
    let clock = foundation_context::Ctx::host().clock();
    let deadline = clock.now() + Duration::from_secs(60);
    loop {
        if let Ok((status, _)) =
            vault_request(&http, "GET", &format!("{address}/v1/sys/health"), None).await
        {
            if status == 200 {
                break;
            }
        }
        if clock.now() > deadline {
            eprintln!("skipping the Vault test: the dev server did not become healthy");
            return None;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // A derived key, because the sealer binds each call to its record with the
    // context.
    let (status, body) = vault_request(
        &http,
        "POST",
        &format!("{address}/v1/transit/keys/{KEY_NAME}"),
        Some("{\"derived\": true}"),
    )
    .await
    .expect("create the transit key");
    if !(200..=299).contains(&status) {
        eprintln!("skipping the Vault test: creating the key failed: {body}");
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
        SecureBytes::new(ROOT_TOKEN.as_bytes().to_vec()),
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
