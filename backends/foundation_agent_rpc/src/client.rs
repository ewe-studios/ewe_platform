//! The client side of agent-rpc's auth: enroll with a token, then call over
//! mutual TLS.
//!
//! WHY: a CLI/SDK should go from "an operator gave me a token" to "a typed,
//! authenticated `AgentServiceClient`" with no manual certificate handling.
//!
//! WHAT: [`generate_csr`] (key stays local), [`enroll`] (token + CSR over a
//! TLS connection pinned to the server's certificate fingerprint),
//! [`ClientIdentity`] (what enrollment yields), and [`connect_mtls`] (the
//! generated `AgentServiceClient` over HTTP/1.1 mutual TLS — Connect by
//! default).
//!
//! HOW: before enrollment the client has no CA, so the bootstrap connection
//! trusts exactly one server certificate: the one whose SHA-256 the operator
//! handed over with the token. Signatures are still verified by rustls.

use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::{ClientOptions, Ctx, H1Transport, Request, Transport};
use foundation_errstacks::ErrorTrace;
use foundation_netio::netcap::ssl::SSLConnector;
use foundation_netio::HttpClientBuilder;
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use zeroize::Zeroizing;

use crate::auth::sha256_fingerprint;
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1::agent_service::AgentServiceClient;
use crate::proto::v1::bootstrap_service::BootstrapServiceClient;
use crate::proto::v1::EnrollRequest;

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
    let err = |what: &str, e: rcgen::Error| {
        ErrorTrace::new(AgentRpcError::Config(format!("{what}: {e}")))
    };
    let key = KeyPair::generate().map_err(|e| err("client key", e))?;
    let mut params =
        CertificateParams::new(Vec::<String>::new()).map_err(|e| err("CSR params", e))?;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, client_name);
    params.distinguished_name = dn;
    let csr = params.serialize_request(&key).map_err(|e| err("CSR", e))?;
    let csr_pem = csr.pem().map_err(|e| err("CSR PEM", e))?;
    Ok((Zeroizing::new(key.serialize_pem()), csr_pem))
}

/// Enroll with the bootstrap listener at `bootstrap_url` (`https://host:port`),
/// trusting only the server certificate whose SHA-256 is `fingerprint_sha256`.
///
/// # Errors
/// The server's status (e.g. `unauthenticated` for a bad or spent token), or
/// [`AgentRpcError::Transport`]/[`AgentRpcError::Config`] mapped into a
/// `ConnectError` for local failures.
pub async fn enroll(
    bootstrap_url: &str,
    fingerprint_sha256: &str,
    token: &str,
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
            Ctx::background().with_deadline(Duration::from_secs(30)),
            Request::new(EnrollRequest {
                token: token.to_string(),
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
/// client plus the identity (keep it to reconnect without the token).
///
/// # Errors
/// `invalid_argument` for a malformed payload; enrollment errors as in
/// [`enroll`]; `failed_precondition` for unusable certificate material.
pub async fn connect_with_payload(
    encoded_payload: &str,
    client_name: &str,
) -> foundation_connectrpc::ConnectResult<(AgentServiceClient, ClientIdentity)> {
    let payload =
        crate::bootstrap::decode(encoded_payload).map_err(|e| e.current_context().clone())?;
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
    let provider = foundation_netio::netcap::ssl::rustls::initialize_tls_provider()
        .map(Arc::new)
        .ok_or_else(|| {
            ErrorTrace::new(AgentRpcError::Config("no rustls crypto provider".into()))
        })?;
    let verifier = PinnedCertVerifier {
        fingerprint_sha256: fingerprint_sha256.to_ascii_lowercase(),
        provider: Arc::clone(&provider),
    };
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| ErrorTrace::new(AgentRpcError::Config(format!("TLS versions: {e}"))))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok(SSLConnector::with_config(Arc::new(config)))
}

/// Trusts exactly one server certificate, identified by its SHA-256; the
/// handshake signatures are verified normally.
#[derive(Debug)]
struct PinnedCertVerifier {
    fingerprint_sha256: String,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for PinnedCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if sha256_fingerprint(end_entity) == self.fingerprint_sha256 {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
