//! The native client: enroll with a signed secret, then call over mutual TLS.
//!
//! WHY: a command-line tool or a software development kit (SDK) goes from "an
//! operator gave me a payload" to "a typed, authenticated `AgentServiceClient`"
//! with no manual certificate handling.
//!
//! WHAT: [`generate_csr`], [`enroll`] (secret and certificate signing request
//! over a pinned TLS connection), [`ClientIdentity`], [`connect_mtls`],
//! [`connect_payload`] (the checked bootstrap payload, both access kinds) and
//! [`connect_with_payload`] (the legacy payload of today).
//!
//! HOW: before enrollment the client has no certificate authority (CA), so the
//! bootstrap connection trusts exactly the server certificate whose SHA-256
//! the operator handed over with the secret. The server proof then proves that
//! the right server holds the payload private key.

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::client_auth::Principal;
use foundation_connectrpc::{ClientOptions, H1Transport, Request, RpcCtx, Transport};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_netio::netcap::ssl::SSLConnector;
use foundation_netio::HttpClientBuilder;
use zeroize::Zeroizing;

use crate::core::bootstrap::{self, Bootstrap, BootstrapAccess, TlsTrust};
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1::agent_service::AgentServiceClient;
use crate::proto::v1::bootstrap_service::BootstrapServiceClient;
use crate::proto::v1::EnrollRequest;

/// A client's certificate, private key and the CA that issued it.
#[derive(Clone)]
pub struct ClientIdentity {
    /// Client certificate (Privacy-Enhanced Mail, PEM).
    pub cert_pem: String,
    /// Client private key (PEM). Secret.
    pub key_pem: Zeroizing<String>,
    /// The server's CA (PEM), trusted for the mutual TLS (mTLS) connection.
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

/// A fresh key pair and a certificate signing request (CSR) for `client_name`.
/// The key never leaves the client.
///
/// # Errors
/// [`AgentRpcError::Config`] when key generation or CSR serialization fails.
pub fn generate_csr(client_name: &str) -> AgentRpcResult<(Zeroizing<String>, String)> {
    foundation_auth::pki::generate_csr(client_name)
        .map_err(|error| ErrorTrace::new(AgentRpcError::Config(format!("client CSR: {error}"))))
}

/// A fresh Ed25519 key pair and a CSR for `client_name`, seeded from `ctx`.
///
/// The same seed gives the same key and the same CSR on every run, so a
/// deterministic simulation test replays the client's identity. The key never
/// leaves the client.
///
/// # Errors
/// [`AgentRpcError::Config`] when key generation or CSR serialization fails.
pub fn generate_csr_seeded(
    client_name: &str,
    ctx: &Ctx,
) -> AgentRpcResult<(Zeroizing<String>, String)> {
    let key =
        foundation_auth::pki::CaKey::generate(foundation_auth::pki::KeyAlgorithm::Ed25519, ctx)
            .map_err(|error| {
                ErrorTrace::new(AgentRpcError::Config(format!("client key: {error}")))
            })?;
    let csr = key
        .csr_pem(client_name)
        .map_err(|error| ErrorTrace::new(AgentRpcError::Config(format!("client CSR: {error}"))))?;
    Ok((key.to_pem(), csr))
}

/// Enroll with the bootstrap listener at `bootstrap_url` (`https://host:port`),
/// trusting only the server certificate whose SHA-256 is `fingerprint_sha256`.
///
/// This is the legacy enrollment path: no nonce and no server proof. Use
/// [`connect_payload`] for the checked payload of feature 03.
///
/// # Errors
/// The server's status (for example `unauthenticated` for a forged, expired or
/// spent secret), or a local transport and configuration error.
pub async fn enroll(
    bootstrap_url: &str,
    fingerprint_sha256: &str,
    secret: &str,
    client_name: &str,
) -> foundation_connectrpc::ConnectResult<ClientIdentity> {
    let (key_pem, csr_pem) =
        generate_csr(client_name).map_err(|error| error.current_context().clone())?;
    let connector =
        pinned_connector(fingerprint_sha256).map_err(|error| error.current_context().clone())?;
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
                credential: Some(
                    crate::proto::v1::__buffa::oneof::enroll_request::Credential::CsrPem(csr_pem),
                ),
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

/// Decode a checked bootstrap payload, check its window, and connect.
///
/// - [`BootstrapAccess::Enroll`] enrolls once: it sends a certificate signing
///   request (CSR) with a nonce, verifies the server proof, and then calls over
///   mutual TLS.
/// - [`BootstrapAccess::Bearer`] calls with the secret in an `Authorization`
///   header on each call.
///
/// # Errors
/// `invalid_argument` for a malformed payload or secret; `unauthenticated` for
/// a secret outside its validity window by the clock of `ctx`; the server
/// status for a refused enrollment; `failed_precondition` for unusable
/// certificate material.
pub async fn connect_payload(
    encoded_payload: &str,
    client_name: &str,
    ctx: &Ctx,
) -> foundation_connectrpc::ConnectResult<AgentServiceClient> {
    let payload =
        bootstrap::decode(encoded_payload).map_err(|error| error.current_context().clone())?;
    check_window(&payload, ctx)?;
    match &payload.access {
        BootstrapAccess::Enroll { token, enroll_url } => {
            let fingerprint = match payload.trust {
                TlsTrust::PinnedSha256(bytes) => Some(hex(&bytes)),
                TlsTrust::SystemRoots | TlsTrust::Plaintext => None,
            };
            let identity = enroll_checked(
                enroll_url.as_str(),
                fingerprint.as_deref(),
                token,
                client_name,
                ctx,
            )
            .await?;
            connect_mtls(payload.url.as_str(), &identity)
                .map_err(|error| error.current_context().clone().into())
        }
        BootstrapAccess::Bearer { token } => {
            let options = ClientOptions::new().with_header(
                foundation_netio::shared::http::SimpleHeader::AUTHORIZATION,
                format!("Bearer {}", token.encode().as_str()),
            );
            let connector = match payload.trust {
                TlsTrust::PinnedSha256(bytes) => Some(client_connector(&hex(&bytes))?),
                TlsTrust::SystemRoots => Some(SSLConnector::with_config(
                    foundation_netio::netcap::ssl::rustls::default_client_config(),
                )),
                TlsTrust::Plaintext => None,
            };
            let mut builder = HttpClientBuilder::new();
            if let Some(connector) = connector {
                builder = builder.with_tls_connector(connector);
            }
            let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(builder.build()));
            AgentServiceClient::new(transport, payload.url.as_str(), options)
        }
    }
}

/// Enroll with a nonce and check the server proof, then return the client
/// identity.
async fn enroll_checked(
    bootstrap_url: &str,
    fingerprint_sha256: Option<&str>,
    token: &foundation_auth::secret::SignedSecret,
    client_name: &str,
    ctx: &Ctx,
) -> foundation_connectrpc::ConnectResult<ClientIdentity> {
    let (key_pem, csr_pem) =
        generate_csr_seeded(client_name, ctx).map_err(|error| error.current_context().clone())?;
    let mut nonce = [0_u8; 32];
    let env = ctx
        .env()
        .ok_or_else(|| {
            ErrorTrace::new(AgentRpcError::Config(
                "the context holds no Env for the enrollment nonce".to_string(),
            ))
        })
        .map_err(|error| error.current_context().clone())?;
    env.entropy()
        .fill(&mut nonce)
        .map_err(|error| {
            ErrorTrace::new(AgentRpcError::Config(format!("enrollment nonce: {error}")))
        })
        .map_err(|error| error.current_context().clone())?;

    let connector = match fingerprint_sha256 {
        Some(fingerprint) => client_connector(fingerprint)?,
        None => SSLConnector::with_config(
            foundation_auth::mtls::client_config_enrollment(None, ctx)
                .map_err(|error| {
                    ErrorTrace::new(AgentRpcError::Config(format!("enrollment TLS: {error}")))
                })
                .map_err(|error| error.current_context().clone())?,
        ),
    };
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
                token: token.encode().to_string(),
                client_name: client_name.to_string(),
                credential: Some(
                    crate::proto::v1::__buffa::oneof::enroll_request::Credential::CsrPem(
                        csr_pem.clone(),
                    ),
                ),
                nonce: nonce.to_vec(),
                ..Default::default()
            }),
        )
        .await?
        .msg;

    let csr_hash = foundation_auth::fingerprint::sha256(csr_pem.as_bytes());
    let proof_ok = token
        .verifying_key()
        .verify_enroll_proof(
            &token.id(),
            &nonce,
            &csr_hash,
            &reply.client_id,
            &reply.server_proof,
        )
        .map_err(|error| ErrorTrace::new(AgentRpcError::Auth(format!("enrollment proof: {error}"))))
        .map_err(|error| error.current_context().clone())?;
    if !proof_ok {
        let error: ErrorTrace<foundation_connectrpc::ConnectError> =
            AgentRpcError::Auth("the server proof of enrollment is not valid".to_string()).into();
        return Err(error);
    }
    Ok(ClientIdentity {
        cert_pem: reply.client_cert_pem,
        key_pem,
        ca_pem: reply.ca_cert_pem,
    })
}

/// The principal of an enrolled client, for a caller that keeps it.
#[must_use]
pub fn principal_of(client_name: &str, fingerprint_sha256: &str) -> Principal {
    Principal {
        subject: client_name.to_string(),
        method: foundation_auth::client_auth::AuthMethod::ClientCertificate {
            fingerprint_sha256: fingerprint_sha256.to_string(),
        },
        roles: vec!["agent-rpc:client".to_string()],
    }
}

/// Connect from the legacy encoded bootstrap payload: decode it, enroll as
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
    let payload = crate::bootstrap::decode(encoded_payload)
        .map_err(|error| error.current_context().clone())?;
    let secret = crate::secret::SignedSecret::decode(&payload.token)
        .map_err(|error| error.current_context().clone())?;
    if !secret.verify_self_signed() {
        return Err(AgentRpcError::Codec("bootstrap secret signature is invalid".into()).into());
    }
    if !secret.is_valid_at(crate::secret::unix_secs(ctx.clock().wall_now())) {
        return Err(AgentRpcError::Auth(
            "bootstrap secret is outside its validity window (expired, or this clock is off)"
                .into(),
        )
        .into());
    }
    let identity = enroll(
        &payload.enroll_url,
        &payload.tls_fingerprint_sha256,
        &payload.token,
        client_name,
    )
    .await?;
    let client =
        connect_mtls(&payload.url, &identity).map_err(|error| error.current_context().clone())?;
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

/// [`connect_mtls`] with explicit options (for example `.with_grpc_web()`).
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
    .map_err(|error| {
        ErrorTrace::new(AgentRpcError::Config(format!(
            "client TLS identity: {error}"
        )))
    })?;
    let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(
        HttpClientBuilder::new()
            .with_tls_connector(connector)
            .build(),
    ));
    AgentServiceClient::new(transport, base_url, options).map_err(|error| {
        ErrorTrace::new(AgentRpcError::Config(error.current_context().to_string()))
    })
}

fn pinned_connector(fingerprint_sha256: &str) -> AgentRpcResult<SSLConnector> {
    client_connector(fingerprint_sha256).map_err(|error| {
        ErrorTrace::new(AgentRpcError::Config(error.current_context().to_string()))
    })
}

fn client_connector(
    fingerprint_sha256: &str,
) -> foundation_connectrpc::ConnectResult<SSLConnector> {
    let config = foundation_auth::mtls::client_config_pinned_default(fingerprint_sha256).map_err(
        |error| -> ErrorTrace<foundation_connectrpc::ConnectError> {
            AgentRpcError::Config(format!("pinned TLS: {error}")).into()
        },
    )?;
    Ok(SSLConnector::with_config(config))
}

/// Check that `payload`'s token is inside its validity window by `ctx`.
fn check_window(payload: &Bootstrap, ctx: &Ctx) -> foundation_connectrpc::ConnectResult<()> {
    let token = match &payload.access {
        BootstrapAccess::Enroll { token, .. } | BootstrapAccess::Bearer { token } => token,
    };
    let now = crate::secret::unix_secs(ctx.clock().wall_now());
    let skew = 300_u64;
    if now.saturating_add(skew) < token.valid_from()
        || now >= token.valid_until().saturating_add(skew)
    {
        return Err(AgentRpcError::Auth(
            "bootstrap secret is outside its validity window (expired, or this clock is off)"
                .into(),
        )
        .into());
    }
    Ok(())
}

/// Lowercase hexadecimal of a SHA-256 value.
fn hex(bytes: &[u8; 32]) -> String {
    use std::fmt::Write as _;

    bytes
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}
