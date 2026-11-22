//! The client side of agent-rpc's auth: enroll with a signed bootstrap
//! secret, then call over mutual TLS.
//!
//! WHY: a CLI/SDK should go from "an operator gave me a payload / QR code" to
//! "a typed, authenticated `AgentServiceClient`" with no manual certificate
//! handling.
//!
//! WHAT: [`generate_csr`] (key stays local), [`enroll`] (secret + CSR over a
//! TLS connection pinned to the server's certificate fingerprint),
//! [`ClientIdentity`] (what enrollment yields), and [`connect_mtls`] (the
//! generated `AgentServiceClient` over HTTP/1.1 mutual TLS — Connect by
//! default).
//!
//! HOW: before enrollment the client has no CA, so the bootstrap connection
//! trusts exactly one server certificate: the one whose SHA-256 the operator
//! handed over with the secret. Signatures are still verified by rustls.
//! [`connect_with_payload`] also checks the secret's own signature and
//! validity window locally, so a truncated or expired secret fails before any
//! network call.

use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::{ClientOptions, H1Transport, Request, RpcCtx, Transport};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_netio::netcap::ssl::SSLConnector;
use foundation_netio::HttpClientBuilder;
use zeroize::Zeroizing;

use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1::agent_service::AgentServiceClient;
use crate::proto::v1::bootstrap_service::BootstrapServiceClient;
use crate::proto::v1::EnrollRequest;
use crate::secret::{unix_secs, SignedSecret};

/// A client's certificate, private key and the CA that issued it.
#[derive(Clone)]
pub struct ClientIdentity {
    /// Client certificate (PEM).
    pub cert_pem: String,
    /// Client private key (PEM). Secret.
    pub key_pem: Zeroizing<String>,
    /// The server's CA (PEM) — trusted for the mTLS connection.
    pub ca_pem: String,
}

impl std::fmt::Debug for ClientIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientIdentity")
            .field("cert_pem", &self.cert_pem)
            .field("key_pem", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// A fresh key pair and a CSR for `client_name`. The key never leaves the
/// client.
///
/// # Errors
/// [`AgentRpcError::Config`] when key generation or CSR serialization fails.
pub fn generate_csr(client_name: &str) -> AgentRpcResult<(Zeroizing<String>, String)> {
    foundation_auth::pki::generate_csr(client_name)
        .map_err(|e| ErrorTrace::new(AgentRpcError::Config(format!("client CSR: {e}"))))
}

/// Enroll with the bootstrap listener at `bootstrap_url` (`https://host:port`),
/// trusting only the server certificate whose SHA-256 is `fingerprint_sha256`.
/// `secret` is the shared form of a [`crate::secret::SignedSecret`].
///
/// # Errors
/// The server's status (e.g. `unauthenticated` for a forged, expired or spent
/// secret), or
/// [`AgentRpcError::Transport`]/[`AgentRpcError::Config`] mapped into a
/// `ConnectError` for local failures.
pub async fn enroll(
    bootstrap_url: &str,
    fingerprint_sha256: &str,
    secret: &str,
    client_name: &str,
) -> foundation_connectrpc::ConnectResult<ClientIdentity> {
    let (key_pem, csr_pem) = generate_csr(client_name).map_err(|e| e.current_context().clone())?;
    let connector =
        pinned_connector(fingerprint_sha256).map_err(|e| e.current_context().clone())?;
    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(
        HttpClientBuilder::new()
            .with_tls_connector(connector)
            .build(),
    ));
    let client = BootstrapServiceClient::new(transport, bootstrap_url, ClientOptions::new())?;
    let reply = client
        .enroll(
            RpcCtx::background().with_deadline(Duration::from_secs(30)),
            Request::new(EnrollRequest {
                token: secret.to_string(),
                client_name: client_name.to_string(),
                csr_pem,
                ..Default::default()
            }),
        )
        .await?
        .msg;
    Ok(ClientIdentity {
        cert_pem: reply.client_cert_pem,
        key_pem,
        ca_pem: reply.ca_cert_pem,
    })
}

/// Connect from an encoded bootstrap payload alone: decode it, enroll as
/// `client_name` (pinned to the payload's fingerprint), and return a mutual-TLS
/// client plus the identity (keep it to reconnect without a secret).
///
/// # Errors
/// `invalid_argument` for a malformed payload or secret (bad signature
/// included); `unauthenticated` for a secret outside its validity window by the
/// `ctx` clock; enrollment errors as in [`enroll`]; `failed_precondition` for
/// unusable certificate material.
pub async fn connect_with_payload(
    encoded_payload: &str,
    client_name: &str,
    ctx: &Ctx,
) -> foundation_connectrpc::ConnectResult<(AgentServiceClient, ClientIdentity)> {
    let payload =
        crate::bootstrap::decode(encoded_payload).map_err(|e| e.current_context().clone())?;
    let secret = SignedSecret::decode(&payload.token).map_err(|e| e.current_context().clone())?;
    if !secret.verify_self_signed() {
        return Err(AgentRpcError::Codec("bootstrap secret signature is invalid".into()).into());
    }
    if !secret.is_valid_at(unix_secs(ctx.clock().wall_now())) {
        return Err(AgentRpcError::Auth(
            "bootstrap secret is outside its validity window (expired, or this clock is off)"
                .into(),
        )
        .into());
    }
    let identity = enroll(
        &format!("https://{}", payload.bootstrap_addr),
        &payload.tls_fingerprint_sha256,
        &payload.token,
        client_name,
    )
    .await?;
    let client = connect_mtls(&format!("https://{}", payload.addr), &identity)
        .map_err(|e| e.current_context().clone())?;
    Ok((client, identity))
}

/// The generated `AgentServiceClient` over HTTP/1.1 mutual TLS (Connect).
///
/// # Errors
/// [`AgentRpcError::Config`] for unusable certificate material.
pub fn connect_mtls(
    base_url: &str,
    identity: &ClientIdentity,
) -> AgentRpcResult<AgentServiceClient> {
    connect_mtls_with(base_url, identity, ClientOptions::new())
}

/// [`connect_mtls`] with explicit options (e.g. `.with_grpc_web()`).
///
/// # Errors
/// [`AgentRpcError::Config`] for unusable certificate material.
pub fn connect_mtls_with(
    base_url: &str,
    identity: &ClientIdentity,
    options: ClientOptions,
) -> AgentRpcResult<AgentServiceClient> {
    let connector = SSLConnector::from_client_mutual_pem(
        Some(identity.ca_pem.as_bytes()),
        identity.cert_pem.as_bytes(),
        identity.key_pem.as_bytes(),
        true,
    )
    .map_err(|e| ErrorTrace::new(AgentRpcError::Config(format!("client TLS identity: {e}"))))?;
    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(
        HttpClientBuilder::new()
            .with_tls_connector(connector)
            .build(),
    ));
    AgentServiceClient::new(transport, base_url, options)
        .map_err(|e| ErrorTrace::new(AgentRpcError::Config(e.current_context().to_string())))
}

fn pinned_connector(fingerprint_sha256: &str) -> AgentRpcResult<SSLConnector> {
    let config = foundation_auth::mtls::client_config_pinned_default(fingerprint_sha256)
        .map_err(|e| ErrorTrace::new(AgentRpcError::Config(format!("pinned TLS: {e}"))))?;
    Ok(SSLConnector::with_config(config))
}
