//! Operator keys, the certificate authority and the native TLS parts.
//!
//! WHY: the host never makes a private key (invariant I1). An operator makes
//! each key with `ssh-keygen` or `openssl`, and the host loads it. A missing key
//! stops the start, and the error names the command that makes it.
//!
//! WHAT: [`OperatorKeys`] (the bootstrap signing key, the CA key and the server
//! key), `load_operator_keys`, `certificate_authority`,
//! `server_identity`, and [`NativeSecurityParts`] (the TLS identity, the trust
//! store, the enrollment service and the client-auth provider).
//!
//! HOW: each key is read from its environment override or its file. The CA
//! certificate and the server certificate are made from the operator keys and
//! written to the state directory. The client-certificate check reads the
//! in-memory trust index only; it does no Input/Output (I/O).

use std::path::Path;
use std::sync::Arc;

use foundation_auth::client_auth::{AccessPolicy, AnyOf, ClientAuth, MtlsAuth, SignedSecretAuth};
use foundation_auth::pki::{CaKey, CertificateAuthority, PkiError, ServerIdentity, CA_CERT_FILE};
use foundation_auth::secret::{
    SecretDomain, SecretIssuer, SecretVerifier, SigningKey, VerifyPolicy,
};
use foundation_auth::trust::{TrustError, TrustedClients};
use foundation_compact::Duration;
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;

use crate::core::enroll::{ClientCertIssuer, EnrollmentService, IssuedClientCert};
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::host::native::config::{NativeHostConfig, NativeSecurity, OperatorKeyConfig};

/// The default lifetime of an enrollment secret: 24 hours, the kubeadm
/// bootstrap-token lifetime.
pub const ENROLL_SECRET_VALIDITY: Duration = Duration::from_hours(24);

/// The operator private keys of one native host.
pub struct OperatorKeys {
    /// The key that signs bootstrap secrets.
    pub signing: SigningKey,
    /// The key of the certificate authority (CA).
    pub ca: CaKey,
    /// The key of the server certificate.
    pub server: CaKey,
}

impl std::fmt::Debug for OperatorKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperatorKeys").finish_non_exhaustive()
    }
}

fn config_error(what: impl Into<String>) -> ErrorTrace<AgentRpcError> {
    AgentRpcError::Config(what.into()).into()
}

fn storage_error(what: impl Into<String>) -> ErrorTrace<AgentRpcError> {
    AgentRpcError::Storage(what.into()).into()
}

fn pki_error(error: &PkiError) -> ErrorTrace<AgentRpcError> {
    AgentRpcError::Config(format!("certificate authority: {error}")).into()
}

fn trust_error(error: &TrustError) -> ErrorTrace<AgentRpcError> {
    match error {
        TrustError::Storage(_) => {
            AgentRpcError::Storage(format!("trusted clients: {error}")).into()
        }
        _ => AgentRpcError::Auth(format!("trusted clients: {error}")).into(),
    }
}

/// Read one operator key.
///
/// The environment override comes first: an inline Privacy-Enhanced Mail (PEM)
/// block, or the path of one. An override that does not load is an error; the
/// host never substitutes another key. Otherwise the file is read. A missing
/// key gives an error that names `make_command`.
fn read_key(config: &OperatorKeyConfig, role: &str, make_command: &str) -> AgentRpcResult<String> {
    if let Some(var) = &config.env_var {
        if let Some(value) = std::env::var_os(var).filter(|value| !value.is_empty()) {
            let value = value.to_string_lossy().into_owned();
            if value.trim_start().starts_with("-----BEGIN") {
                return Ok(value);
            }
            return std::fs::read_to_string(value.trim())
                .map_err(|error| config_error(format!("{role} key from ${var}: {error}")));
        }
    }
    match std::fs::read_to_string(&config.path) {
        Ok(pem) => Ok(pem),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(config_error(format!(
            "missing {role} key at {}: make one with `{make_command}`",
            config.path.display()
        ))),
        Err(error) => Err(config_error(format!(
            "{role} key {}: {error}",
            config.path.display()
        ))),
    }
}

/// Load the bootstrap signing key, the CA key and the server key.
///
/// # Errors
/// [`AgentRpcError::Config`] when a key is missing or does not parse. The
/// message names the `ssh-keygen` or `openssl` command that makes the key.
pub fn load_operator_keys(config: &NativeHostConfig) -> AgentRpcResult<OperatorKeys> {
    let signing_source = config.signing_key_source();
    let signing_pem = read_key(
        &signing_source,
        "bootstrap signing",
        &format!(
            "openssl genpkey -algorithm ED25519 -out {}",
            signing_source.path.display()
        ),
    )?;
    let signing = SigningKey::from_pkcs8_pem(&signing_pem, None).map_err(|error| {
        config_error(format!(
            "bootstrap signing key {}: {error}; make a new one with `openssl genpkey -algorithm ED25519 -out {}`",
            signing_source.path.display(),
            signing_source.path.display()
        ))
    })?;

    let ca_source = config.ca_key_source();
    let ca_pem = read_key(
        &ca_source,
        "certificate authority",
        &format!(
            "openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out {}",
            ca_source.path.display()
        ),
    )?;
    let ca = CaKey::from_pem(&ca_pem).map_err(|error| {
        config_error(format!(
            "certificate authority key {}: {error}; make a new one with `openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out {}`",
            ca_source.path.display(),
            ca_source.path.display()
        ))
    })?;

    let server_source = config.server_key_source();
    let server_pem = read_key(
        &server_source,
        "server",
        &format!(
            "openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out {}",
            server_source.path.display()
        ),
    )?;
    let server = CaKey::from_pem(&server_pem).map_err(|error| {
        config_error(format!(
            "server key {}: {error}; make a new one with `openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out {}`",
            server_source.path.display(),
            server_source.path.display()
        ))
    })?;

    Ok(OperatorKeys {
        signing,
        ca,
        server,
    })
}

/// Build the certificate authority from the operator CA key.
///
/// The CA certificate is read from `state_dir` when it exists and matches the
/// key. Otherwise the host makes one from the key and writes it. The host never
/// makes a key.
///
/// # Errors
/// [`AgentRpcError::Config`] when the certificate cannot be read or made;
/// [`AgentRpcError::Storage`] when the certificate cannot be written.
pub fn certificate_authority(
    config: &NativeHostConfig,
    key: CaKey,
    ctx: &Ctx,
) -> AgentRpcResult<Arc<CertificateAuthority>> {
    let cert_path = config.state_dir.join(CA_CERT_FILE);
    let existing = match std::fs::read_to_string(&cert_path) {
        Ok(pem) => Some(pem),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(storage_error(format!(
                "certificate authority certificate {}: {error}",
                cert_path.display()
            )));
        }
    };
    let (ca, fresh) = CertificateAuthority::from_key(key, existing.as_deref(), &config.ca, ctx)
        .map_err(|error| pki_error(&error))?;
    if let Some(pem) = fresh {
        write_public_file(&cert_path, pem.as_bytes())?;
    }
    Ok(Arc::new(ca))
}

/// Make the server certificate from the operator server key.
///
/// The certificate is written to `state_dir` on every start, so a renewal and a
/// changed name list take effect. The host never makes a key.
///
/// # Errors
/// [`AgentRpcError::Config`] when the certificate cannot be made;
/// [`AgentRpcError::Storage`] when it cannot be written.
pub fn server_identity(
    config: &NativeHostConfig,
    ca: &CertificateAuthority,
    key: CaKey,
    ctx: &Ctx,
) -> AgentRpcResult<ServerIdentity> {
    let identity = ca
        .issue_server_identity(
            &key,
            &config.server_identity.names,
            config.server_identity.lifetime,
        )
        .map_err(|error| pki_error(&error))?;
    let path = config.state_dir.join(SERVER_CERT_FILE);
    write_public_file(&path, identity.cert_pem().as_bytes())?;
    Ok(identity)
}

/// File name of the server certificate inside the state directory.
pub const SERVER_CERT_FILE: &str = "server-cert.pem";

/// Write a public file atomically, readable by every user on Unix.
fn write_public_file(path: &Path, bytes: &[u8]) -> AgentRpcResult<()> {
    use std::io::Write;

    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .map_err(|error| storage_error(format!("creating {}: {error}", dir.display())))?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o644);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|error| storage_error(format!("creating {}: {error}", tmp.display())))?;
    file.write_all(bytes)
        .map_err(|error| storage_error(format!("writing {}: {error}", tmp.display())))?;
    file.sync_all()
        .map_err(|error| storage_error(format!("syncing {}: {error}", tmp.display())))?;
    drop(file);
    std::fs::rename(&tmp, path)
        .map_err(|error| storage_error(format!("installing {}: {error}", path.display())))
}

/// The certificate issuer seam over a native certificate authority.
struct CaClientIssuer {
    ca: Arc<CertificateAuthority>,
}

impl std::fmt::Debug for CaClientIssuer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaClientIssuer").finish_non_exhaustive()
    }
}

impl ClientCertIssuer for CaClientIssuer {
    fn sign_client_csr(
        &self,
        csr_pem: &str,
        client_name: &str,
        lifetime: Duration,
    ) -> AgentRpcResult<IssuedClientCert> {
        let issued = self
            .ca
            .sign_client_csr(csr_pem, client_name, lifetime)
            .map_err(|error| match error {
                PkiError::BadCsr(_) | PkiError::BadClientName(_) => {
                    ErrorTrace::new(AgentRpcError::Auth(error.to_string()))
                }
                _ => pki_error(&error),
            })?;
        Ok(IssuedClientCert {
            cert_pem: issued.cert_pem,
            fingerprint_sha256: issued.fingerprint_sha256,
            not_after: issued.not_after,
        })
    }

    fn ca_cert_pem(&self) -> &str {
        self.ca.ca_cert_pem()
    }
}

/// The authentication parts of a secure native host.
pub struct MtlsParts {
    /// The server certificate and its private key.
    pub identity: ServerIdentity,
    /// The certificate authority.
    pub ca: Arc<CertificateAuthority>,
    /// The trusted clients.
    pub trust: Arc<TrustedClients>,
    /// Enrollment and refresh.
    pub enrollment: Arc<EnrollmentService>,
    /// The provider that checks the client certificate on each call.
    pub client_auth: Arc<dyn ClientAuth>,
    /// Lowercase hexadecimal SHA-256 of the server certificate.
    pub fingerprint_sha256: String,
}

impl std::fmt::Debug for MtlsParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MtlsParts")
            .field("fingerprint_sha256", &self.fingerprint_sha256)
            .finish_non_exhaustive()
    }
}

/// The security configuration of a running native host.
pub enum NativeSecurityParts {
    /// Mutual TLS (mTLS), with enrollment.
    Mtls(Box<MtlsParts>),
    /// Development only: no TLS and no auth.
    Insecure,
}

impl std::fmt::Debug for NativeSecurityParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mtls(parts) => f.debug_tuple("Mtls").field(parts).finish(),
            Self::Insecure => f.write_str("Insecure"),
        }
    }
}

/// Build the TLS identity, the trust store, the enrollment service and the
/// client-auth provider of a secure host.
///
/// `accept_access_secrets` adds a bearer-secret provider next to `MtlsAuth`.
///
/// # Errors
/// [`AgentRpcError::Config`] when the enrollment service cannot read the
/// effect environment.
pub fn build_mtls(
    config: &NativeHostConfig,
    signing: SigningKey,
    ca: Arc<CertificateAuthority>,
    identity: ServerIdentity,
    trust: Arc<TrustedClients>,
    ctx: &Ctx,
    audience: &str,
) -> AgentRpcResult<NativeSecurityParts> {
    let mut enroll_policy = VerifyPolicy::enroll();
    enroll_policy.audience = Some(audience.to_string());
    enroll_policy.clock_skew = config.enroll.clock_skew;
    let verifying = signing.verifying_key().clone();
    let verifier = SecretVerifier::new(
        SecretDomain::AGENT_RPC,
        vec![verifying.clone()],
        ctx.clone(),
        enroll_policy,
    );
    let signer = SecretIssuer::new(
        SecretDomain::AGENT_RPC,
        signing,
        ctx.clone(),
        ENROLL_SECRET_VALIDITY,
    );
    let issuer: Arc<dyn ClientCertIssuer> = Arc::new(CaClientIssuer {
        ca: Arc::clone(&ca),
    });
    let enrollment = Arc::new(
        EnrollmentService::new(
            verifier,
            signer,
            Some(issuer),
            Arc::clone(&trust),
            ctx.clone(),
            config.enroll.clone(),
        )
        .map_err(|error| {
            config_error(format!("enrollment service: {}", error.current_context()))
        })?,
    );

    let mtls: Arc<dyn ClientAuth> = Arc::new(MtlsAuth::new(Arc::clone(&trust), ctx));
    let client_auth: Arc<dyn ClientAuth> = match config.security {
        NativeSecurity::Mtls {
            accept_access_secrets: true,
        } => {
            let access_verifier = SecretVerifier::new(
                SecretDomain::AGENT_RPC,
                vec![verifying],
                ctx.clone(),
                VerifyPolicy::access(audience.to_string()),
            );
            let bearer: Arc<dyn ClientAuth> = Arc::new(SignedSecretAuth::new(
                access_verifier,
                Arc::clone(&trust),
                AccessPolicy::default(),
            ));
            Arc::new(AnyOf::new(vec![mtls, bearer]))
        }
        _ => mtls,
    };

    let fingerprint_sha256 = identity.fingerprint_sha256().to_string();
    Ok(NativeSecurityParts::Mtls(Box::new(MtlsParts {
        identity,
        ca,
        trust,
        enrollment,
        client_auth,
        fingerprint_sha256,
    })))
}
