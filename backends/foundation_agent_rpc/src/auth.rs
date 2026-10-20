//! Client-facing auth: bootstrap token → client certificate → mutual TLS.
//!
//! WHY: agent-rpc owns how its clients authenticate (spec 65: the proxy never
//! sees this channel). A client starts with nothing but a secret token handed to
//! it out of band; after one enrollment it holds a certificate, and from then on
//! every call is authenticated by mutual TLS.
//!
//! WHAT:
//! - [`BootstrapToken`] — 256-bit secret, never printed, compared in constant
//!   time, consumed on use.
//! - [`CertificateAuthority`] — issues the server certificate and signs client
//!   CSRs (rcgen, ring backend).
//! - [`MtlsConfig`] — the CA plus certificate lifetimes, server names and
//!   enrollment policy (limits are fields with documented defaults).
//! - [`EnrollmentService`] — the generated `BootstrapService`: verify the token,
//!   sign the CSR.
//! - TLS server configs for the two listeners (server-auth bootstrap, mutual-TLS
//!   agent listener).
//!
//! HOW: the agent listener's rustls config requires a client certificate that
//! chains to the CA, so a call without one fails in the handshake — no request
//! reaches a handler. The bootstrap listener serves only `Enroll`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use foundation_connectrpc::{ConnectResult, Ctx, Request, Response};
use foundation_errstacks::ErrorTrace;
use foundation_netio::netcap::ssl::SSLAcceptor;
use rcgen::{
    BasicConstraints, CertificateParams, CertificateSigningRequestParams, DistinguishedName,
    DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1::bootstrap_service::BootstrapService;
use crate::proto::v1::{EnrollRequest, EnrollResponse};

// ---------------------------------------------------------------------------
// BootstrapToken

/// The secret a client presents once to obtain its certificate.
///
/// `Debug`/`Display` never show the secret; the bytes are zeroed on drop.
#[derive(Clone)]
pub struct BootstrapToken(Zeroizing<String>);

impl BootstrapToken {
    /// Minimum length of a caller-supplied secret, in characters: 32
    /// base64url characters carry 192 bits.
    const MIN_SECRET_LEN: usize = 32;

    /// A fresh token: 32 bytes (256 bits) of OS entropy, base64url, unpadded.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when the entropy source fails.
    pub fn generate() -> AgentRpcResult<Self> {
        let mut bytes = Zeroizing::new([0u8; 32]);
        foundation_compact::entropy::fill(bytes.as_mut()).map_err(|e| {
            ErrorTrace::new(AgentRpcError::Config(format!(
                "entropy for the bootstrap token: {e}"
            )))
        })?;
        Ok(Self(Zeroizing::new(
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes.as_ref()),
        )))
    }

    /// Use an operator-supplied secret.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for a secret shorter than 32 characters.
    pub fn from_secret(secret: impl Into<String>) -> AgentRpcResult<Self> {
        let secret = Zeroizing::new(secret.into());
        if secret.chars().count() < Self::MIN_SECRET_LEN {
            return Err(AgentRpcError::Config(format!(
                "bootstrap token must be at least {} characters",
                Self::MIN_SECRET_LEN
            ))
            .into());
        }
        Ok(Self(secret))
    }

    /// The secret itself — for the bootstrap payload handed to the operator.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Constant-time comparison with a presented secret (length is not
    /// secret: every generated token has the same length).
    #[must_use]
    pub fn matches(&self, presented: &str) -> bool {
        let (a, b) = (self.0.as_bytes(), presented.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }
}

impl std::fmt::Debug for BootstrapToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BootstrapToken(<redacted>)")
    }
}

impl std::fmt::Display for BootstrapToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted bootstrap token>")
    }
}

// ---------------------------------------------------------------------------
// CertificateAuthority

/// The CA that signs agent-rpc's server certificate and its clients'
/// certificates.
pub struct CertificateAuthority {
    cert: rcgen::Certificate,
    key: KeyPair,
    cert_pem: String,
}

impl std::fmt::Debug for CertificateAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertificateAuthority")
            .finish_non_exhaustive()
    }
}

/// A server certificate and its key, ready for a TLS listener.
pub struct ServerIdentity {
    cert_der: CertificateDer<'static>,
    key_der: Zeroizing<Vec<u8>>,
    fingerprint_sha256: String,
}

impl std::fmt::Debug for ServerIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerIdentity")
            .field("fingerprint_sha256", &self.fingerprint_sha256)
            .finish_non_exhaustive()
    }
}

impl ServerIdentity {
    /// Lowercase hex SHA-256 of the certificate (DER) — what clients pin
    /// before they have the CA.
    #[must_use]
    pub fn fingerprint_sha256(&self) -> &str {
        &self.fingerprint_sha256
    }
}

/// Lowercase hex SHA-256 of `der`.
#[must_use]
pub fn sha256_fingerprint(der: &[u8]) -> String {
    use std::fmt::Write;
    Sha256::digest(der)
        .iter()
        .fold(String::with_capacity(64), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

fn tls_err(context: &str, e: impl std::fmt::Display) -> ErrorTrace<AgentRpcError> {
    ErrorTrace::new(AgentRpcError::Config(format!("{context}: {e}")))
}

fn validity(params: &mut CertificateParams, lifetime: Duration) {
    let now = time::OffsetDateTime::now_utc();
    // Tolerate small clock skew between the server and its clients.
    params.not_before = now - time::Duration::minutes(5);
    params.not_after =
        now + time::Duration::seconds(i64::try_from(lifetime.as_secs()).unwrap_or(i64::MAX / 2));
}

impl CertificateAuthority {
    /// A new self-signed CA, valid for 10 years (mkcert's root lifetime).
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when key generation or signing fails.
    pub fn generate(common_name: &str) -> AgentRpcResult<Self> {
        let key = KeyPair::generate().map_err(|e| tls_err("CA key", e))?;
        let mut params =
            CertificateParams::new(Vec::<String>::new()).map_err(|e| tls_err("CA params", e))?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, common_name);
        params.distinguished_name = dn;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        validity(&mut params, Duration::from_secs(10 * 365 * 24 * 3600));
        let cert = params
            .self_signed(&key)
            .map_err(|e| tls_err("CA self-sign", e))?;
        let cert_pem = cert.pem();
        Ok(Self {
            cert,
            key,
            cert_pem,
        })
    }

    /// Load a persisted CA (its certificate and private key, PEM).
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when the PEM cannot be parsed.
    pub fn from_pem(cert_pem: &str, key_pem: &str) -> AgentRpcResult<Self> {
        let key = KeyPair::from_pem(key_pem).map_err(|e| tls_err("CA key PEM", e))?;
        let params =
            CertificateParams::from_ca_cert_pem(cert_pem).map_err(|e| tls_err("CA cert PEM", e))?;
        // rcgen signs with a `Certificate`; rebuilding it from the same
        // subject and key yields signatures the original certificate verifies.
        let cert = params
            .self_signed(&key)
            .map_err(|e| tls_err("CA reload", e))?;
        Ok(Self {
            cert,
            key,
            cert_pem: cert_pem.to_string(),
        })
    }

    /// The CA certificate (PEM) clients trust.
    #[must_use]
    pub fn ca_cert_pem(&self) -> &str {
        &self.cert_pem
    }

    /// The CA private key (PEM), for persisting the CA. Treat as a secret.
    #[must_use]
    pub fn key_pem(&self) -> Zeroizing<String> {
        Zeroizing::new(self.key.serialize_pem())
    }

    /// Issue the server certificate for `names` (DNS names or IP literals).
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when generation or signing fails.
    pub fn issue_server_cert(
        &self,
        names: &[String],
        lifetime: Duration,
    ) -> AgentRpcResult<ServerIdentity> {
        let key = KeyPair::generate().map_err(|e| tls_err("server key", e))?;
        let mut params =
            CertificateParams::new(names.to_vec()).map_err(|e| tls_err("server names", e))?;
        let mut dn = DistinguishedName::new();
        dn.push(
            DnType::CommonName,
            names.first().map_or("agent-rpc", String::as_str),
        );
        params.distinguished_name = dn;
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        validity(&mut params, lifetime);
        let cert = params
            .signed_by(&key, &self.cert, &self.key)
            .map_err(|e| tls_err("server cert", e))?;
        let cert_der = cert.der().clone();
        Ok(ServerIdentity {
            fingerprint_sha256: sha256_fingerprint(&cert_der),
            cert_der,
            key_der: Zeroizing::new(key.serialize_der()),
        })
    }

    /// Sign a client's CSR. Only the CSR's public key is used: the subject is
    /// `client_name`, the certificate is a non-CA client-auth leaf, and its
    /// lifetime is ours — nothing the client asks for is copied over.
    ///
    /// # Errors
    /// [`AgentRpcError::Auth`] for an invalid CSR or client name;
    /// [`AgentRpcError::Config`] when signing fails.
    pub fn sign_client_csr(
        &self,
        csr_pem: &str,
        client_name: &str,
        lifetime: Duration,
    ) -> AgentRpcResult<String> {
        validate_client_name(client_name)?;
        let csr = CertificateSigningRequestParams::from_pem(csr_pem)
            .map_err(|e| ErrorTrace::new(AgentRpcError::Auth(format!("invalid CSR: {e}"))))?;
        let mut params = CertificateParams::new(Vec::<String>::new())
            .map_err(|e| tls_err("client params", e))?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, client_name);
        params.distinguished_name = dn;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        validity(&mut params, lifetime);
        let cert = params
            .signed_by(&csr.public_key, &self.cert, &self.key)
            .map_err(|e| tls_err("client cert", e))?;
        Ok(cert.pem())
    }

    fn root_store(&self) -> AgentRpcResult<rustls::RootCertStore> {
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(self.cert.der().clone())
            .map_err(|e| tls_err("CA root store", e))?;
        Ok(roots)
    }
}

/// Client names become certificate subjects and log fields: 1..=64 of
/// `[A-Za-z0-9 ._-]`.
fn validate_client_name(name: &str) -> AgentRpcResult<()> {
    let ok = (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" ._-".contains(&b));
    if ok {
        Ok(())
    } else {
        Err(AgentRpcError::Auth("client name must be 1-64 of [A-Za-z0-9 ._-]".into()).into())
    }
}

// ---------------------------------------------------------------------------
// MtlsConfig

/// Mutual-TLS settings for a secure server.
#[derive(Debug, Clone)]
pub struct MtlsConfig {
    ca: Arc<CertificateAuthority>,
    server_names: Vec<String>,
    server_cert_lifetime: Duration,
    client_cert_lifetime: Duration,
    max_enrollments: u32,
    bootstrap_bind: Option<String>,
}

impl MtlsConfig {
    /// Defaults:
    /// - `server_names`: `localhost`, `127.0.0.1`, `::1`;
    /// - certificate lifetimes: 90 days (Let's Encrypt's leaf lifetime);
    /// - `max_enrollments`: 1 — a token enrolls one client, then is spent;
    /// - bootstrap listener: the agent listener's host, an ephemeral port.
    #[must_use]
    pub fn new(ca: Arc<CertificateAuthority>) -> Self {
        Self {
            ca,
            server_names: vec!["localhost".into(), "127.0.0.1".into(), "::1".into()],
            server_cert_lifetime: Duration::from_secs(90 * 24 * 3600),
            client_cert_lifetime: Duration::from_secs(90 * 24 * 3600),
            max_enrollments: 1,
            bootstrap_bind: None,
        }
    }

    /// DNS names / IPs the server certificate covers.
    #[must_use]
    pub fn with_server_names(mut self, names: Vec<String>) -> Self {
        self.server_names = names;
        self
    }

    /// Server certificate lifetime.
    #[must_use]
    pub fn with_server_cert_lifetime(mut self, lifetime: Duration) -> Self {
        self.server_cert_lifetime = lifetime;
        self
    }

    /// Lifetime of issued client certificates.
    #[must_use]
    pub fn with_client_cert_lifetime(mut self, lifetime: Duration) -> Self {
        self.client_cert_lifetime = lifetime;
        self
    }

    /// How many clients one bootstrap token may enroll.
    #[must_use]
    pub fn with_max_enrollments(mut self, max: u32) -> Self {
        self.max_enrollments = max;
        self
    }

    /// `host:port` for the bootstrap listener.
    #[must_use]
    pub fn with_bootstrap_bind(mut self, addr: impl Into<String>) -> Self {
        self.bootstrap_bind = Some(addr.into());
        self
    }

    /// The CA.
    #[must_use]
    pub fn ca(&self) -> &Arc<CertificateAuthority> {
        &self.ca
    }

    pub(crate) fn bootstrap_bind(&self) -> Option<&str> {
        self.bootstrap_bind.as_deref()
    }
}

// ---------------------------------------------------------------------------
// Enrollment

/// The generated `BootstrapService`: token + CSR → client certificate.
pub struct EnrollmentService {
    token: Mutex<Option<(BootstrapToken, u32)>>,
    ca: Arc<CertificateAuthority>,
    client_cert_lifetime: Duration,
    max_enrollments: u32,
}

impl std::fmt::Debug for EnrollmentService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnrollmentService").finish_non_exhaustive()
    }
}

impl EnrollmentService {
    /// Enroll clients presenting `token`, under `config`'s policy.
    #[must_use]
    pub fn new(token: BootstrapToken, config: &MtlsConfig) -> Self {
        Self {
            token: Mutex::new(Some((token, config.max_enrollments))),
            ca: Arc::clone(&config.ca),
            client_cert_lifetime: config.client_cert_lifetime,
            max_enrollments: config.max_enrollments,
        }
    }

    /// Replace the token (and reset its enrollment count); the old one stops
    /// working immediately.
    pub fn rotate(&self, token: BootstrapToken) {
        let mut slot = self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *slot = Some((token, self.max_enrollments));
    }

    /// Verify the token, consume one use, sign the CSR.
    ///
    /// # Errors
    /// [`AgentRpcError::Auth`] for a wrong, spent or rotated-out token, or an
    /// invalid CSR / client name.
    pub fn enroll_client(&self, req: &EnrollRequest) -> AgentRpcResult<EnrollResponse> {
        validate_client_name(&req.client_name)?;
        {
            let mut slot = self
                .token
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let accepted =
                matches!(&*slot, Some((token, uses)) if *uses > 0 && token.matches(&req.token));
            if !accepted {
                tracing::warn!(client = %req.client_name, "enrollment refused: bad or spent token");
                return Err(AgentRpcError::Auth("invalid or spent bootstrap token".into()).into());
            }
            if let Some((_, uses)) = slot.as_mut() {
                *uses -= 1;
                if *uses == 0 {
                    *slot = None;
                }
            }
        }
        let cert =
            self.ca
                .sign_client_csr(&req.csr_pem, &req.client_name, self.client_cert_lifetime)?;
        tracing::info!(client = %req.client_name, "client enrolled");
        Ok(EnrollResponse {
            client_cert_pem: cert,
            ca_cert_pem: self.ca.ca_cert_pem().to_string(),
            ..Default::default()
        })
    }
}

impl BootstrapService for EnrollmentService {
    fn enroll(
        &self,
        _ctx: Ctx,
        request: Request<EnrollRequest>,
    ) -> impl std::future::Future<Output = ConnectResult<Response<EnrollResponse>>> + Send {
        let result = self
            .enroll_client(&request.msg)
            .map(Response::new)
            .map_err(|e| e.current_context().clone().into());
        async move { result }
    }
}

// ---------------------------------------------------------------------------
// TLS listeners

/// The two TLS acceptors of a secure server plus what clients pin.
pub(crate) struct TlsListeners {
    pub(crate) agent: Arc<SSLAcceptor>,
    pub(crate) bootstrap: Arc<SSLAcceptor>,
    pub(crate) fingerprint_sha256: String,
}

fn provider() -> AgentRpcResult<Arc<rustls::crypto::CryptoProvider>> {
    foundation_netio::netcap::ssl::rustls::initialize_tls_provider()
        .map(Arc::new)
        .ok_or_else(|| AgentRpcError::Config("no rustls crypto provider".into()).into())
}

pub(crate) fn tls_listeners(config: &MtlsConfig) -> AgentRpcResult<TlsListeners> {
    let identity = config
        .ca
        .issue_server_cert(&config.server_names, config.server_cert_lifetime)?;
    let provider = provider()?;
    let chain = vec![identity.cert_der.clone()];
    let key = || PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(identity.key_der.to_vec()));

    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(config.ca.root_store()?),
        Arc::clone(&provider),
    )
    .build()
    .map_err(|e| tls_err("client verifier", e))?;
    let agent = rustls::ServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| tls_err("TLS versions", e))?
        .with_client_cert_verifier(verifier)
        .with_single_cert(chain.clone(), key())
        .map_err(|e| tls_err("agent listener cert", e))?;
    let bootstrap = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| tls_err("TLS versions", e))?
        .with_no_client_auth()
        .with_single_cert(chain, key())
        .map_err(|e| tls_err("bootstrap listener cert", e))?;

    Ok(TlsListeners {
        agent: Arc::new(SSLAcceptor::with_config(Arc::new(agent))),
        bootstrap: Arc::new(SSLAcceptor::with_config(Arc::new(bootstrap))),
        fingerprint_sha256: identity.fingerprint_sha256,
    })
}
