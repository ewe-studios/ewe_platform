//! Server and client Transport Layer Security (TLS) configurations, and their
//! limits.

use std::sync::Arc;

use foundation_compact::Duration;
use foundation_context::Ctx;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::server::WebPkiClientVerifier;
use rustls::time_provider::TimeProvider;
use rustls::{DigitallySignedStruct, RootCertStore, SignatureScheme};

use super::time::CtxTimeProvider;
use super::verifier::ClientCertVerifier;
use crate::pki::{sha256_hex, CertificateAuthority, PkiError, ServerIdentity};
use crate::trust::TrustIndex;

/// The configuration of the server certificate.
///
/// Every value is a field with a `with_` builder and a documented default.
#[derive(Debug, Clone)]
pub struct ServerTlsConfig {
    /// Domain Name System (DNS) names and IP addresses in the certificate.
    /// Default `localhost`, `127.0.0.1` and `::1`.
    pub names: Vec<String>,
    /// Certificate lifetime. Default 90 days, the lifetime of a Let's Encrypt
    /// certificate.
    pub lifetime: Duration,
    /// Make a new certificate when less than this time is left. Default one
    /// third of [`ServerTlsConfig::lifetime`], when certbot renews.
    pub renew_before: Duration,
}

impl Default for ServerTlsConfig {
    fn default() -> Self {
        Self {
            names: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "::1".to_string(),
            ],
            lifetime: Duration::from_hours(90 * 24),
            renew_before: Duration::from_hours(30 * 24),
        }
    }
}

impl ServerTlsConfig {
    /// Set the names.
    #[must_use]
    pub fn with_names(mut self, names: Vec<String>) -> Self {
        self.names = names;
        self
    }

    /// Set the lifetime. The default `renew_before` follows it.
    #[must_use]
    pub fn with_lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = lifetime;
        self.renew_before = lifetime / 3;
        self
    }

    /// Set the renewal window.
    #[must_use]
    pub fn with_renew_before(mut self, renew_before: Duration) -> Self {
        self.renew_before = renew_before;
        self
    }
}

/// The mutual-TLS (mTLS) policy that the certificate authority (CA) and the
/// enrollment service share.
///
/// Every value is a field with a `with_` builder and a documented default.
#[derive(Debug, Clone)]
pub struct MtlsConfig {
    /// The server certificate configuration.
    pub server: ServerTlsConfig,
    /// Lifetime of an issued client certificate. Default 30 days, the access-
    /// secret window of the agent RPC.
    pub client_cert_lifetime: Duration,
    /// How many clients one bootstrap secret may enroll. Default 1.
    pub max_enrollments: u32,
}

impl Default for MtlsConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl MtlsConfig {
    /// The default lifetime of a client certificate: 30 days.
    pub const DEFAULT_CLIENT_CERT_LIFETIME: Duration = Duration::from_hours(30 * 24);

    /// The default enrollment limit: one client per bootstrap secret.
    pub const DEFAULT_MAX_ENROLLMENTS: u32 = 1;

    /// A configuration with the defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            server: ServerTlsConfig::default(),
            client_cert_lifetime: Self::DEFAULT_CLIENT_CERT_LIFETIME,
            max_enrollments: Self::DEFAULT_MAX_ENROLLMENTS,
        }
    }

    /// Set the server certificate configuration.
    #[must_use]
    pub fn with_server(mut self, server: ServerTlsConfig) -> Self {
        self.server = server;
        self
    }

    /// Set the server certificate names.
    #[must_use]
    pub fn with_server_names(mut self, names: Vec<String>) -> Self {
        self.server.names = names;
        self
    }

    /// Set the client certificate lifetime.
    #[must_use]
    pub fn with_client_cert_lifetime(mut self, lifetime: Duration) -> Self {
        self.client_cert_lifetime = lifetime;
        self
    }

    /// Set the enrollment limit.
    #[must_use]
    pub fn with_max_enrollments(mut self, max: u32) -> Self {
        self.max_enrollments = max;
        self
    }
}

/// The `rustls` crypto provider.
///
/// The provider is passed explicitly, so two libraries in one process do not
/// race to install a global provider.
#[must_use]
pub fn crypto_provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn time_provider(ctx: &Ctx) -> Arc<dyn TimeProvider> {
    Arc::new(CtxTimeProvider(ctx.clock()))
}

fn tls_error(what: &str, e: impl std::fmt::Display) -> PkiError {
    PkiError::Ca(format!("{what}: {e}"))
}

fn certs_from_pem(pem: &str) -> Result<Vec<CertificateDer<'static>>, PkiError> {
    let mut reader = std::io::Cursor::new(pem.as_bytes());
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| tls_error("certificate PEM", e))
}

fn key_from_pem(pem: &str) -> Result<PrivateKeyDer<'static>, PkiError> {
    let mut reader = std::io::Cursor::new(pem.as_bytes());
    rustls_pemfile::private_key(&mut reader)
        .map_err(|e| tls_error("private key PEM", e))?
        .ok_or_else(|| PkiError::Key("the private key PEM holds no key".to_string()))
}

/// A server configuration that requires a client certificate from `ca` that is
/// in `index`.
///
/// The configuration reads the time from the `Ctx` clock.
///
/// # Errors
/// [`PkiError`] when the certificate authority root store, a protocol version
/// or the certificate chain is invalid.
pub fn server_config_mutual(
    identity: &ServerIdentity,
    ca: &CertificateAuthority,
    index: TrustIndex,
    ctx: &Ctx,
) -> Result<Arc<rustls::ServerConfig>, PkiError> {
    let provider = crypto_provider();
    let chain = WebPkiClientVerifier::builder_with_provider(
        Arc::new(ca.root_store()?),
        Arc::clone(&provider),
    )
    .build()
    .map_err(|e| tls_error("client verifier", e))?;
    let verifier = Arc::new(ClientCertVerifier::new(chain, index));
    let config = rustls::ServerConfig::builder_with_details(provider, time_provider(ctx))
        .with_safe_default_protocol_versions()
        .map_err(|e| tls_error("TLS versions", e))?
        .with_client_cert_verifier(verifier)
        .with_single_cert(vec![identity.cert_der().clone()], identity.key_der())
        .map_err(|e| tls_error("server certificate", e))?;
    Ok(Arc::new(config))
}

/// A server configuration with no client certificate, for the enrollment
/// listener.
///
/// # Errors
/// [`PkiError`] when a protocol version or the certificate chain is invalid.
pub fn server_config_server_auth(
    identity: &ServerIdentity,
    ctx: &Ctx,
) -> Result<Arc<rustls::ServerConfig>, PkiError> {
    let config = rustls::ServerConfig::builder_with_details(crypto_provider(), time_provider(ctx))
        .with_safe_default_protocol_versions()
        .map_err(|e| tls_error("TLS versions", e))?
        .with_no_client_auth()
        .with_single_cert(vec![identity.cert_der().clone()], identity.key_der())
        .map_err(|e| tls_error("server certificate", e))?;
    Ok(Arc::new(config))
}

/// A client configuration that trusts exactly one server certificate, by its
/// SHA-256 fingerprint. The handshake signatures are still verified.
///
/// # Errors
/// [`PkiError`] when a protocol version fails.
pub fn client_config_pinned(
    fingerprint_sha256: &str,
    ctx: &Ctx,
) -> Result<Arc<rustls::ClientConfig>, PkiError> {
    let provider = crypto_provider();
    let verifier = PinnedCertVerifier {
        fingerprint_sha256: fingerprint_sha256.to_ascii_lowercase(),
        provider: Arc::clone(&provider),
    };
    let config = rustls::ClientConfig::builder_with_details(provider, time_provider(ctx))
        .with_safe_default_protocol_versions()
        .map_err(|e| tls_error("TLS versions", e))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// [`client_config_pinned`] with the default `rustls` time source.
///
/// Use it from a client that has no `Ctx`; the pinned verifier never reads the
/// time.
///
/// # Errors
/// [`PkiError`] when a protocol version fails.
pub fn client_config_pinned_default(
    fingerprint_sha256: &str,
) -> Result<Arc<rustls::ClientConfig>, PkiError> {
    let provider = crypto_provider();
    let verifier = PinnedCertVerifier {
        fingerprint_sha256: fingerprint_sha256.to_ascii_lowercase(),
        provider: Arc::clone(&provider),
    };
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| tls_error("TLS versions", e))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// A client configuration with a client certificate that trusts `ca_pem`.
///
/// # Errors
/// [`PkiError`] when the certificate, the key or a protocol version is invalid.
pub fn client_config_mutual(
    ca_pem: &str,
    cert_pem: &str,
    key_pem: &str,
    ctx: &Ctx,
) -> Result<Arc<rustls::ClientConfig>, PkiError> {
    let mut roots = RootCertStore::empty();
    for cert in certs_from_pem(ca_pem)? {
        roots
            .add(cert)
            .map_err(|e| tls_error("certificate authority", e))?;
    }
    let chain = certs_from_pem(cert_pem)?;
    let key = key_from_pem(key_pem)?;
    let config = rustls::ClientConfig::builder_with_details(crypto_provider(), time_provider(ctx))
        .with_safe_default_protocol_versions()
        .map_err(|e| tls_error("TLS versions", e))?
        .with_root_certificates(roots)
        .with_client_auth_cert(chain, key)
        .map_err(|e| tls_error("client certificate", e))?;
    Ok(Arc::new(config))
}

/// Trusts exactly one server certificate, identified by its SHA-256. The
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
        if sha256_hex(end_entity.as_ref()) == self.fingerprint_sha256 {
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
