//! The certificate authority (CA): load or generate the CA, validate a
//! certificate signing request (CSR), issue a certificate and renew one.
//!
//! WHY: a proxy, an agent server and a sandbox service all need one CA and one
//! certificate vocabulary (spec 87). Without it every consumer writes its own,
//! and a checker cannot compare two implementations.
//!
//! WHAT: [`CaStore`] (file or memory), [`CertificateAuthority`], [`CaConfig`],
//! [`KeyAlgorithm`], [`CertificateRequest`], [`IssuedCertificate`],
//! [`ServerIdentity`] and [`CaKey`].
//!
//! HOW: `rcgen` builds the X.509 structure and this module signs it. The time
//! comes from the `Ctx` clock, never from the system clock, so a deterministic
//! simulation test (DST) run crosses a validity window at a known point.
//!
//! Native only: the module reads key files and uses `rustls`.

mod error;
mod identity;
mod key;
mod store;

pub use error::PkiError;
pub use identity::{IssuedCertificate, ServerIdentity};
pub use key::{CaKey, KeyAlgorithm};
pub use store::{CaStore, CA_CERT_FILE, CA_KEY_FILE};

use std::sync::Arc;

use foundation_compact::{Duration, SystemTime};
use foundation_context::{Clock, Ctx};
use rcgen::{
    BasicConstraints, CertificateParams, CertificateSigningRequestParams, DistinguishedName,
    DnType, ExtendedKeyUsagePurpose, IsCa, KeyUsagePurpose,
};
use sha2::{Digest, Sha256};

/// The default common name of a generated CA.
pub const DEFAULT_CA_NAME: &str = "ewe mutual TLS CA";

/// The default lifetime of a CA: 10 years, the `mkcert` root lifetime.
pub const DEFAULT_CA_LIFETIME: Duration = Duration::from_hours(10 * 365 * 24);

/// The default backdate of a certificate: 5 minutes, the clock skew that MIT
/// Kerberos allows by default (`clockskew`).
pub const DEFAULT_BACKDATE: Duration = Duration::from_mins(5);

/// The default lifetime of a client certificate: 30 days, the access-secret
/// window of the agent RPC.
pub const DEFAULT_CLIENT_LIFETIME: Duration = Duration::from_hours(30 * 24);

/// Lowercase hexadecimal SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// The configuration of a certificate authority (CA).
///
/// Every value is a field with a `with_` builder and a documented default.
#[derive(Debug, Clone)]
pub struct CaConfig {
    /// The common name of the CA certificate.
    pub common_name: String,
    /// The algorithm of the operator's CA key.
    pub key_algorithm: KeyAlgorithm,
    /// The lifetime of a generated CA certificate. Default 10 years.
    pub ca_lifetime: Duration,
    /// How far before the current time `not_before` starts, for a client whose
    /// clock is behind. Default 5 minutes.
    pub backdate: Duration,
}

impl Default for CaConfig {
    fn default() -> Self {
        Self {
            common_name: DEFAULT_CA_NAME.to_string(),
            key_algorithm: KeyAlgorithm::EcdsaP256,
            ca_lifetime: DEFAULT_CA_LIFETIME,
            backdate: DEFAULT_BACKDATE,
        }
    }
}

impl CaConfig {
    /// A configuration for `common_name` with the defaults of [`Default`].
    #[must_use]
    pub fn new(common_name: impl Into<String>) -> Self {
        Self {
            common_name: common_name.into(),
            ..Self::default()
        }
    }

    /// Set the common name.
    #[must_use]
    pub fn with_common_name(mut self, common_name: impl Into<String>) -> Self {
        self.common_name = common_name.into();
        self
    }

    /// Set the CA key algorithm.
    #[must_use]
    pub fn with_key_algorithm(mut self, algorithm: KeyAlgorithm) -> Self {
        self.key_algorithm = algorithm;
        self
    }

    /// Set the CA lifetime.
    #[must_use]
    pub fn with_ca_lifetime(mut self, lifetime: Duration) -> Self {
        self.ca_lifetime = lifetime;
        self
    }

    /// Set the backdate.
    #[must_use]
    pub fn with_backdate(mut self, backdate: Duration) -> Self {
        self.backdate = backdate;
        self
    }
}

/// The configuration of one issued client certificate.
#[derive(Debug, Clone)]
pub struct CertificateRequest {
    /// The certificate signing request (CSR) in Privacy-Enhanced Mail (PEM)
    /// form. Only its public key is used.
    pub csr_pem: String,
    /// The subject common name of the client.
    pub client_name: String,
    /// The lifetime of the certificate. Default 30 days.
    pub lifetime: Duration,
}

impl CertificateRequest {
    /// A request for `client_name`, with the default lifetime.
    #[must_use]
    pub fn new(csr_pem: impl Into<String>, client_name: impl Into<String>) -> Self {
        Self {
            csr_pem: csr_pem.into(),
            client_name: client_name.into(),
            lifetime: DEFAULT_CLIENT_LIFETIME,
        }
    }

    /// Set the lifetime.
    #[must_use]
    pub fn with_lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = lifetime;
        self
    }
}

/// The issuance settings of one client certificate. The design of
/// `requirements.md` section 4.5 names this type `IssueConfig`; it is
/// [`CertificateRequest`] plus its content.
pub type IssueConfig = CertificateRequest;

/// Issues the server certificate and signs client certificate signing requests
/// (CSRs).
pub struct CertificateAuthority {
    cert: rcgen::Certificate,
    cert_pem: String,
    key: CaKey,
    clock: Arc<dyn Clock>,
    config: CaConfig,
}

impl std::fmt::Debug for CertificateAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertificateAuthority")
            .field("common_name", &self.config.common_name)
            .field("key_algorithm", &self.config.key_algorithm)
            .finish_non_exhaustive()
    }
}

impl CertificateAuthority {
    /// A new CA with a fresh key and a self-signed certificate.
    ///
    /// The method is a convenience for a test and a short-lived process. A host
    /// uses [`CaStore::load_or_generate`], which keeps the CA across restarts.
    ///
    /// # Errors
    /// [`PkiError`] when the key or the certificate cannot be made.
    pub fn generate(config: &CaConfig, ctx: &Ctx) -> Result<(Self, String), PkiError> {
        let key = CaKey::generate(config.key_algorithm, ctx)?;
        let (ca, fresh) = Self::from_key(key, None, config, ctx)?;
        Ok((ca, fresh.unwrap_or_default()))
    }

    /// The CA over `key`. The method reuses `existing_cert_pem` when its public
    /// key is the key's public key. Otherwise it makes a new self-signed
    /// certificate with dates from the `Ctx` clock, and returns its Privacy-
    /// Enhanced Mail (PEM) form so that the host writes it.
    ///
    /// # Errors
    /// [`PkiError`] when the existing certificate cannot be parsed or a new one
    /// cannot be signed.
    pub fn from_key(
        key: CaKey,
        existing_cert_pem: Option<&str>,
        config: &CaConfig,
        ctx: &Ctx,
    ) -> Result<(Self, Option<String>), PkiError> {
        if let Some(existing) = existing_cert_pem {
            if certificate_matches_key(existing, &key)? {
                let params = CertificateParams::from_ca_cert_pem(existing)
                    .map_err(|e| PkiError::Ca(format!("parsing the CA certificate: {e}")))?;
                let cert = params
                    .self_signed(key.pair())
                    .map_err(|e| PkiError::Ca(format!("loading the CA: {e}")))?;
                return Ok((
                    Self {
                        cert,
                        cert_pem: existing.to_string(),
                        key,
                        clock: ctx.clock(),
                        config: config.clone(),
                    },
                    None,
                ));
            }
        }

        let mut params = CertificateParams::new(Vec::<String>::new())
            .map_err(|e| PkiError::Ca(format!("CA parameters: {e}")))?;
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, config.common_name.clone());
        params.distinguished_name = name;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        set_validity(&mut params, ctx, config, config.ca_lifetime)?;
        let cert = params
            .self_signed(key.pair())
            .map_err(|e| PkiError::Ca(format!("signing the CA certificate: {e}")))?;
        let cert_pem = cert.pem();
        Ok((
            Self {
                cert,
                cert_pem: cert_pem.clone(),
                key,
                clock: ctx.clock(),
                config: config.clone(),
            },
            Some(cert_pem),
        ))
    }

    /// The CA certificate in Privacy-Enhanced Mail (PEM) form.
    #[must_use]
    pub fn ca_cert_pem(&self) -> &str {
        &self.cert_pem
    }

    /// The CA certificate in Distinguished Encoding Rules (DER) form.
    #[must_use]
    pub fn ca_cert_der(&self) -> &rustls::pki_types::CertificateDer<'static> {
        self.cert.der()
    }

    /// The CA private key in Privacy-Enhanced Mail (PEM) form. Treat the result
    /// as a secret.
    #[must_use]
    pub fn key_pem(&self) -> zeroize::Zeroizing<String> {
        self.key.to_pem()
    }

    /// The CA configuration.
    #[must_use]
    pub fn config(&self) -> &CaConfig {
        &self.config
    }

    /// The CA as a `rustls` root store.
    ///
    /// # Errors
    /// [`PkiError::Ca`] when `rustls` refuses the certificate.
    pub fn root_store(&self) -> Result<rustls::RootCertStore, PkiError> {
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(self.cert.der().clone())
            .map_err(|e| PkiError::Ca(format!("CA root store: {e}")))?;
        Ok(roots)
    }

    /// A server certificate for `names` over `server_key`.
    ///
    /// `names` holds Domain Name System (DNS) names and IP addresses.
    ///
    /// # Errors
    /// [`PkiError`] when a name is invalid or the certificate cannot be signed.
    pub fn issue_server_identity(
        &self,
        server_key: &CaKey,
        names: &[String],
        lifetime: Duration,
    ) -> Result<ServerIdentity, PkiError> {
        let mut params = CertificateParams::new(names.to_vec())
            .map_err(|e| PkiError::Ca(format!("server names: {e}")))?;
        let mut name = DistinguishedName::new();
        name.push(
            DnType::CommonName,
            names.first().map_or("localhost", String::as_str),
        );
        params.distinguished_name = name;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        set_validity(&mut params, &self.ctx(), &self.config, lifetime)?;
        let cert = params
            .signed_by(server_key.pair(), &self.cert, self.key.pair())
            .map_err(|e| PkiError::Ca(format!("signing the server certificate: {e}")))?;
        let cert_der = cert.der().clone();
        let fingerprint_sha256 = sha256_hex(cert_der.as_ref());
        Ok(ServerIdentity::new(
            cert_der,
            cert.pem(),
            zeroize::Zeroizing::new(server_key.pair().serialize_der()),
            fingerprint_sha256,
        ))
    }

    /// Sign a client certificate signing request (CSR).
    ///
    /// Only the public key of the CSR is used. The subject is `client_name`,
    /// the certificate is a non-CA leaf with the `clientAuth` extended key
    /// usage, and the lifetime is this module's, never the CSR's.
    ///
    /// # Errors
    /// [`PkiError::BadClientName`] for an invalid name, [`PkiError::BadCsr`]
    /// for a malformed CSR or a bad CSR signature, and [`PkiError::Ca`] when
    /// the certificate cannot be signed.
    pub fn sign_client_csr(
        &self,
        csr_pem: &str,
        client_name: &str,
        lifetime: Duration,
    ) -> Result<IssuedCertificate, PkiError> {
        validate_client_name(client_name)?;
        let request = CertificateSigningRequestParams::from_pem(csr_pem)
            .map_err(|e| PkiError::BadCsr(format!("invalid or badly signed CSR: {e}")))?;
        let mut params = CertificateParams::new(Vec::<String>::new())
            .map_err(|e| PkiError::Ca(format!("client parameters: {e}")))?;
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, client_name);
        params.distinguished_name = name;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        let not_after = set_validity(&mut params, &self.ctx(), &self.config, lifetime)?;
        let cert = params
            .signed_by(&request.public_key, &self.cert, self.key.pair())
            .map_err(|e| PkiError::Ca(format!("signing the client certificate: {e}")))?;
        Ok(IssuedCertificate {
            fingerprint_sha256: sha256_hex(cert.der().as_ref()),
            cert_pem: cert.pem(),
            not_after,
        })
    }

    /// Sign a [`CertificateRequest`].
    ///
    /// # Errors
    /// As [`sign_client_csr`](Self::sign_client_csr).
    pub fn issue_client(
        &self,
        request: &CertificateRequest,
    ) -> Result<IssuedCertificate, PkiError> {
        self.sign_client_csr(&request.csr_pem, &request.client_name, request.lifetime)
    }

    /// Issue a new certificate for an enrolled client from a fresh CSR. The
    /// operation is [`sign_client_csr`](Self::sign_client_csr) with a name that
    /// the caller already knows.
    ///
    /// # Errors
    /// As [`sign_client_csr`](Self::sign_client_csr).
    pub fn renew(
        &self,
        csr_pem: &str,
        client_name: &str,
        lifetime: Duration,
    ) -> Result<IssuedCertificate, PkiError> {
        self.sign_client_csr(csr_pem, client_name, lifetime)
    }

    fn ctx(&self) -> Ctx {
        Ctx::fixed(Arc::clone(&self.clock))
    }
}

/// Client names become certificate subjects and log fields: 1..=64 bytes of
/// `[A-Za-z0-9 ._-]`.
///
/// # Errors
/// [`PkiError::BadClientName`] when the name is empty, too long or holds an
/// unsafe byte.
pub fn validate_client_name(name: &str) -> Result<(), PkiError> {
    let ok = (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b" ._-".contains(&byte));
    if ok {
        Ok(())
    } else {
        Err(PkiError::BadClientName(
            "a client name must be 1-64 bytes of [A-Za-z0-9 ._-]".to_string(),
        ))
    }
}

/// A fresh key and a certificate signing request (CSR) for `client_name`. The
/// private key stays with the caller.
///
/// # Errors
/// [`PkiError::BadClientName`] for an invalid name, and [`PkiError::Key`] when
/// the key or the CSR cannot be made.
pub fn generate_csr(client_name: &str) -> Result<(zeroize::Zeroizing<String>, String), PkiError> {
    validate_client_name(client_name)?;
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
        .map_err(|e| PkiError::Key(e.to_string()))?;
    let mut params = CertificateParams::new(Vec::<String>::new())
        .map_err(|e| PkiError::Ca(format!("CSR parameters: {e}")))?;
    let mut name = DistinguishedName::new();
    name.push(DnType::CommonName, client_name);
    params.distinguished_name = name;
    let csr = params
        .serialize_request(&key)
        .map_err(|e| PkiError::Ca(format!("CSR: {e}")))?;
    let csr_pem = csr
        .pem()
        .map_err(|e| PkiError::Ca(format!("CSR PEM: {e}")))?;
    Ok((zeroize::Zeroizing::new(key.serialize_pem()), csr_pem))
}

/// Set `not_before` and `not_after` from the `Ctx` clock and return `not_after`
/// as Unix seconds.
fn set_validity(
    params: &mut CertificateParams,
    ctx: &Ctx,
    config: &CaConfig,
    lifetime: Duration,
) -> Result<u64, PkiError> {
    let now = offset_datetime(ctx.clock().wall_now())?;
    let back = time::Duration::seconds(clamp_seconds(config.backdate));
    let life = time::Duration::seconds(clamp_seconds(lifetime));
    params.not_before = now
        .checked_sub(back)
        .ok_or_else(|| PkiError::Ca("the certificate backdate overflows the time range".into()))?;
    params.not_after = now
        .checked_add(life)
        .ok_or_else(|| PkiError::Ca("the certificate lifetime overflows the time range".into()))?;
    Ok(u64::try_from(params.not_after.unix_timestamp()).unwrap_or(0))
}

/// Convert a wall-clock time to a `time` value.
fn offset_datetime(time: SystemTime) -> Result<time::OffsetDateTime, PkiError> {
    time::OffsetDateTime::from_unix_timestamp(foundation_compact::time::unix_seconds(time)).map_err(
        |e| {
            PkiError::Ca(format!(
                "the wall clock is outside the certificate range: {e}"
            ))
        },
    )
}

/// Clamp a duration to the whole-seconds range that `time` accepts.
fn clamp_seconds(duration: Duration) -> i64 {
    i64::try_from(duration.as_secs()).unwrap_or(i64::MAX / 2)
}

/// Whether the public key in `cert_pem` is the public key of `key`.
fn certificate_matches_key(cert_pem: &str, key: &CaKey) -> Result<bool, PkiError> {
    use rustls_pemfile::certs;

    let mut reader = std::io::Cursor::new(cert_pem.as_bytes());
    let Some(cert) = certs(&mut reader).next() else {
        return Ok(false);
    };
    let cert = cert.map_err(|e| PkiError::Store(format!("parsing the CA certificate: {e}")))?;
    let (_, parsed) = x509_parser::parse_x509_certificate(cert.as_ref())
        .map_err(|e| PkiError::Store(format!("parsing the CA certificate: {e}")))?;
    Ok(parsed.tbs_certificate.subject_pki.raw == key.public_key_der().as_slice())
}
