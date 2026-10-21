//! Client-facing auth: signed bootstrap secret → client certificate → mutual
//! TLS, with a durable record of every enrolled client.
//!
//! WHY: agent-rpc owns how its clients authenticate (spec 65: the proxy never
//! sees this channel). A client starts with nothing but a signed, time-boxed
//! secret handed to it out of band; after one enrollment it holds a
//! certificate, and from then on every call is authenticated by mutual TLS
//! against the trusted-client store.
//!
//! WHAT:
//! - [`CertificateAuthority`] — issues the server certificate and signs client
//!   CSRs (rcgen, ring backend); persisted in the state directory by
//!   [`CertificateAuthority::load_or_generate`].
//! - [`MtlsConfig`] — the CA, the bootstrap-secret signer
//!   ([`crate::secret::BootstrapSecrets`]), the trusted-client store
//!   ([`crate::trust::TrustedClientStore`]), certificate lifetimes, server
//!   names and enrollment policy (limits are fields with documented defaults).
//!   [`MtlsConfig::from_state_dir`] loads or creates all of it in one place.
//! - [`EnrollmentService`] — the generated `BootstrapService`: verify the
//!   secret, spend one use, sign the CSR, record the client.
//! - TLS server configs for the two listeners (server-auth bootstrap, mutual-TLS
//!   agent listener).
//!
//! HOW: the agent listener's rustls config requires a client certificate that
//! chains to the CA **and** is in the trusted-client store, not revoked — so a
//! call without one fails in the handshake and no request reaches a handler.
//! The bootstrap listener serves only `Enroll`.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use foundation_connectrpc::{ConnectResult, Ctx, Request, Response};
use foundation_errstacks::ErrorTrace;
use foundation_netio::netcap::ssl::SSLAcceptor;
use rcgen::{
    BasicConstraints, CertificateParams, CertificateSigningRequestParams, DistinguishedName,
    DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose,
};
use rustls::client::danger::HandshakeSignatureValid;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName as TlsDistinguishedName, SignatureScheme};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1::bootstrap_service::BootstrapService;
use crate::proto::v1::{EnrollRequest, EnrollResponse};
use crate::secret::{
    unix_secs, write_secret_file, BootstrapSecrets, SignedSecret, SigningKey, SigningKeyConfig,
};
use crate::trust::{TrustedClient, TrustedClientStore};

/// File name of the CA certificate inside a state directory.
pub const CA_CERT_FILE: &str = "ca-cert.pem";

/// File name of the CA private key inside a state directory.
pub const CA_KEY_FILE: &str = "ca-key.pem";

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

/// Set the certificate's validity; returns `not_after` (Unix seconds).
fn validity(params: &mut CertificateParams, lifetime: Duration) -> u64 {
    let now = time::OffsetDateTime::now_utc();
    // Tolerate small clock skew between the server and its clients.
    params.not_before = now - time::Duration::minutes(5);
    params.not_after =
        now + time::Duration::seconds(i64::try_from(lifetime.as_secs()).unwrap_or(i64::MAX / 2));
    u64::try_from(params.not_after.unix_timestamp()).unwrap_or(0)
}

/// A client certificate issued by [`CertificateAuthority::sign_client_csr`].
#[derive(Debug, Clone)]
pub struct IssuedClientCert {
    /// The certificate (PEM).
    pub cert_pem: String,
    /// Lowercase hex SHA-256 of the certificate (DER) — its id in the
    /// trusted-client store.
    pub fingerprint_sha256: String,
    /// Expiry (Unix seconds).
    pub not_after: u64,
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
        let _ = validity(&mut params, Duration::from_secs(10 * 365 * 24 * 3600));
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

    /// Load the CA persisted in `state_dir` ([`CA_CERT_FILE`] + [`CA_KEY_FILE`]),
    /// or generate one there (named `common_name`, files readable by the
    /// owner only) when neither file exists. The CA must outlive restarts:
    /// every client certificate chains to it.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when only one of the two files exists, a file
    /// cannot be read or written, or the PEM is invalid.
    pub fn load_or_generate(state_dir: impl AsRef<Path>, common_name: &str) -> AgentRpcResult<Self> {
        let dir = state_dir.as_ref();
        let (cert_path, key_path) = (dir.join(CA_CERT_FILE), dir.join(CA_KEY_FILE));
        let read = |p: &Path| {
            std::fs::read_to_string(p).map_err(|e| tls_err(&format!("reading {}", p.display()), e))
        };
        match (cert_path.exists(), key_path.exists()) {
            (true, true) => {
                let key_pem = Zeroizing::new(read(&key_path)?);
                Self::from_pem(&read(&cert_path)?, &key_pem)
            }
            (false, false) => {
                let ca = Self::generate(common_name)?;
                write_secret_file(&key_path, ca.key_pem().as_bytes())?;
                write_secret_file(&cert_path, ca.ca_cert_pem().as_bytes())?;
                tracing::info!(dir = %dir.display(), "generated a new agent-rpc CA");
                Ok(ca)
            }
            _ => Err(AgentRpcError::Config(format!(
                "{} holds only one of {CA_CERT_FILE} and {CA_KEY_FILE}",
                dir.display()
            ))
            .into()),
        }
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
        let _ = validity(&mut params, lifetime);
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
    ) -> AgentRpcResult<IssuedClientCert> {
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
        let not_after = validity(&mut params, lifetime);
        let cert = params
            .signed_by(&csr.public_key, &self.cert, &self.key)
            .map_err(|e| tls_err("client cert", e))?;
        Ok(IssuedClientCert {
            cert_pem: cert.pem(),
            fingerprint_sha256: sha256_fingerprint(cert.der()),
            not_after,
        })
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
    secrets: Arc<BootstrapSecrets>,
    trusted: Arc<TrustedClientStore>,
    server_names: Vec<String>,
    server_cert_lifetime: Duration,
    client_cert_lifetime: Duration,
    max_enrollments: u32,
    bootstrap_bind: Option<String>,
}

impl MtlsConfig {
    /// Common name of a CA generated by [`from_state_dir`](Self::from_state_dir).
    pub const DEFAULT_CA_NAME: &'static str = "ewe agent-rpc CA";

    /// The CA signs certificates, `secrets` issues and verifies bootstrap
    /// secrets, `trusted` records enrolled clients.
    ///
    /// Defaults:
    /// - `server_names`: `localhost`, `127.0.0.1`, `::1`;
    /// - certificate lifetimes: 90 days (Let's Encrypt's leaf lifetime);
    /// - `max_enrollments`: 1 — a secret enrolls one client, then is spent;
    /// - bootstrap listener: the agent listener's host, an ephemeral port.
    #[must_use]
    pub fn new(
        ca: Arc<CertificateAuthority>,
        secrets: Arc<BootstrapSecrets>,
        trusted: Arc<TrustedClientStore>,
    ) -> Self {
        Self {
            ca,
            secrets,
            trusted,
            server_names: vec!["localhost".into(), "127.0.0.1".into(), "::1".into()],
            server_cert_lifetime: Duration::from_secs(90 * 24 * 3600),
            client_cert_lifetime: Duration::from_secs(90 * 24 * 3600),
            max_enrollments: 1,
            bootstrap_bind: None,
        }
    }

    /// Everything from one state directory (see
    /// [`crate::secret::default_state_dir`]), created as needed:
    /// - the CA ([`CertificateAuthority::load_or_generate`]);
    /// - the signing key ([`SigningKey::load_or_generate`] with
    ///   [`SigningKeyConfig::in_state_dir`]: the `EWE_AGENT_RPC_SIGNING_KEY`
    ///   override, else a key file generated on first start);
    /// - the trusted-client store (SQLite,
    ///   [`TrustedClientStore::default_path`]).
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] / [`AgentRpcError::Storage`] when any of the
    /// three cannot be loaded or created.
    pub async fn from_state_dir(state_dir: impl AsRef<Path>) -> AgentRpcResult<Self> {
        Self::from_state_dir_with(
            state_dir.as_ref(),
            &SigningKeyConfig::in_state_dir(state_dir.as_ref()),
        )
        .await
    }

    /// [`from_state_dir`](Self::from_state_dir) with an explicit signing-key
    /// source.
    ///
    /// # Errors
    /// As [`from_state_dir`](Self::from_state_dir).
    pub async fn from_state_dir_with(
        state_dir: &Path,
        signing_key: &SigningKeyConfig,
    ) -> AgentRpcResult<Self> {
        let ca = CertificateAuthority::load_or_generate(state_dir, Self::DEFAULT_CA_NAME)?;
        let (key, origin) = SigningKey::load_or_generate(signing_key)?;
        tracing::debug!(?origin, "bootstrap signing key ready");
        let trusted =
            TrustedClientStore::open_sqlite(TrustedClientStore::default_path(state_dir)).await?;
        Ok(Self::new(
            Arc::new(ca),
            Arc::new(BootstrapSecrets::new(key)),
            trusted,
        ))
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

    /// How many clients one bootstrap secret may enroll.
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

    /// The bootstrap-secret signer.
    #[must_use]
    pub fn secrets(&self) -> &Arc<BootstrapSecrets> {
        &self.secrets
    }

    /// The trusted-client store.
    #[must_use]
    pub fn trusted_clients(&self) -> &Arc<TrustedClientStore> {
        &self.trusted
    }

    pub(crate) fn bootstrap_bind(&self) -> Option<&str> {
        self.bootstrap_bind.as_deref()
    }
}

// ---------------------------------------------------------------------------
// Enrollment

/// The generated `BootstrapService`: signed secret + CSR → client certificate.
pub struct EnrollmentService {
    secrets: Arc<BootstrapSecrets>,
    trusted: Arc<TrustedClientStore>,
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
    /// Enroll clients under `config`'s policy.
    #[must_use]
    pub fn new(config: &MtlsConfig) -> Self {
        Self {
            secrets: Arc::clone(&config.secrets),
            trusted: Arc::clone(&config.trusted),
            ca: Arc::clone(&config.ca),
            client_cert_lifetime: config.client_cert_lifetime,
            max_enrollments: config.max_enrollments,
        }
    }

    /// A new bootstrap secret, valid for the signer's default validity.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when entropy or signing fails.
    pub fn issue_secret(&self) -> AgentRpcResult<SignedSecret> {
        self.secrets.issue()
    }

    /// A new bootstrap secret valid for `validity`.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when entropy or signing fails.
    pub fn issue_secret_for(&self, validity: Duration) -> AgentRpcResult<SignedSecret> {
        self.secrets.issue_for(validity)
    }

    /// The trusted-client store (list, revoke).
    #[must_use]
    pub fn trusted_clients(&self) -> &Arc<TrustedClientStore> {
        &self.trusted
    }

    /// Verify the secret, sign the CSR, spend one use of the secret, record
    /// the client.
    ///
    /// # Errors
    /// [`AgentRpcError::Auth`] for a malformed, foreign, tampered, expired or
    /// spent secret, or an invalid CSR / client name;
    /// [`AgentRpcError::Storage`] when the store fails.
    pub async fn enroll_client(&self, req: &EnrollRequest) -> AgentRpcResult<EnrollResponse> {
        validate_client_name(&req.client_name)?;
        let now = unix_secs(SystemTime::now());
        let verified = self.secrets.verify_at(&req.token, now).inspect_err(|e| {
            tracing::warn!(client = %req.client_name, reason = %e.current_context(), "enrollment refused");
        })?;
        // Signing first validates the CSR, so a bad CSR does not spend a use.
        let issued =
            self.ca
                .sign_client_csr(&req.csr_pem, &req.client_name, self.client_cert_lifetime)?;
        if !self
            .trusted
            .consume_secret_use(&verified.id, verified.valid_until, self.max_enrollments, now)
            .await?
        {
            tracing::warn!(client = %req.client_name, "enrollment refused: bootstrap secret already used");
            return Err(AgentRpcError::Auth("bootstrap secret already used".into()).into());
        }
        self.trusted
            .record_enrollment(
                TrustedClient {
                    fingerprint_sha256: issued.fingerprint_sha256.clone(),
                    client_name: req.client_name.clone(),
                    enrolled_at: now,
                    expires_at: issued.not_after,
                    last_seen_at: None,
                    revoked_at: None,
                },
                &issued.cert_pem,
            )
            .await?;
        tracing::info!(
            client = %req.client_name,
            fingerprint = %issued.fingerprint_sha256,
            "client enrolled"
        );
        Ok(EnrollResponse {
            client_cert_pem: issued.cert_pem,
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
        async move {
            self.enroll_client(&request.msg)
                .await
                .map(Response::new)
                .map_err(|e| e.current_context().clone().into())
        }
    }
}

// ---------------------------------------------------------------------------
// Client-certificate verification

/// WebPKI chain verification to the CA, then the trusted-client store: the
/// certificate must have been issued here and be neither revoked nor expired.
/// A successful handshake updates the client's last-seen time in the
/// background (the verifier cannot await).
#[derive(Debug)]
struct TrustedClientVerifier {
    chain: Arc<dyn ClientCertVerifier>,
    trusted: Arc<TrustedClientStore>,
}

impl ClientCertVerifier for TrustedClientVerifier {
    fn offer_client_auth(&self) -> bool {
        self.chain.offer_client_auth()
    }

    fn client_auth_mandatory(&self) -> bool {
        self.chain.client_auth_mandatory()
    }

    fn root_hint_subjects(&self) -> &[TlsDistinguishedName] {
        self.chain.root_hint_subjects()
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        self.chain
            .verify_client_cert(end_entity, intermediates, now)?;
        let fingerprint = sha256_fingerprint(end_entity);
        let now = now.as_secs();
        match self.trusted.get(&fingerprint) {
            Some(client) if client.is_active_at(now) => {
                let trusted = Arc::clone(&self.trusted);
                let seen = async move {
                    if let Err(e) = trusted.mark_seen(&fingerprint, now).await {
                        tracing::warn!(error = %e.current_context(), "recording a client handshake failed");
                    }
                };
                if let Err(e) = foundation_core::valtron::send(foundation_core::valtron::from_future(seen)) {
                    tracing::warn!(error = %e, "scheduling the last-seen update failed");
                }
                Ok(ClientCertVerified::assertion())
            }
            Some(client) if client.revoked_at.is_some() => {
                tracing::warn!(client = %client.client_name, "mTLS refused: client revoked");
                Err(rustls::Error::InvalidCertificate(rustls::CertificateError::Revoked))
            }
            Some(client) => {
                tracing::warn!(client = %client.client_name, "mTLS refused: client certificate expired");
                Err(rustls::Error::InvalidCertificate(rustls::CertificateError::Expired))
            }
            None => {
                tracing::warn!(%fingerprint, "mTLS refused: certificate not in the trusted-client store");
                Err(rustls::Error::InvalidCertificate(
                    rustls::CertificateError::ApplicationVerificationFailure,
                ))
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.chain.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.chain.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.chain.supported_verify_schemes()
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

    let chain_verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(config.ca.root_store()?),
        Arc::clone(&provider),
    )
    .build()
    .map_err(|e| tls_err("client verifier", e))?;
    let verifier = Arc::new(TrustedClientVerifier {
        chain: chain_verifier,
        trusted: Arc::clone(&config.trusted),
    });
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
