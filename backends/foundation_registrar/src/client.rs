//! The registration client: enrollment, mutual TLS and renewal.
//!
//! WHY: the sidecar proves its identity only on its link to the proxy
//! registration authority. It shows a one-time enrollment token, gets a client
//! certificate, and from then on uses mutual TLS (mTLS) (spec 82 feature 01,
//! section 4.6).
//!
//! WHAT: [`RegistrationClient`], [`RegistrationApi`], [`IdentityStore`] and
//! the generated `RegistrationService` client.
//!
//! HOW: the client is built from the proxy's `.proto` by this crate's build
//! script. Enrollment uses server-auth Transport Layer Security (TLS) with the
//! certificate pin when configured, verifies the server proof with the token
//! key, writes the certificate, key and authority in `state_dir` with mode
//! 0600, and deletes the token file. Every later call uses mTLS, or a Unix
//! socket when the proxy URL is `unix://`.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use base64::Engine as _;
use foundation_auth::mtls::{client_config_enrollment, client_config_mutual};
use foundation_auth::pki;
use foundation_auth::secret::{DecodeLimits, SignedSecret};
use foundation_compact::rng::Rng;
use foundation_connectrpc::{
    ClientOptions, Code, ConnectError, H1Transport, Request, RpcCtx, Transport,
};
use foundation_context::Ctx;
use foundation_core::valtron::sleep_async;
use foundation_netio::netcap::ssl::SSLConnector;
use foundation_netio::HttpClientBuilder;
use futures_util::future::{select, Either};
use futures_util::FutureExt;
use sha2::{Digest, Sha256};

use crate::config::RegistrarConfig;
use crate::error::RegistrarError;
use crate::proto::v1::registrar_enrollment_service::RegistrarEnrollmentServiceClient;
use crate::proto::v1::registration_service::RegistrationServiceClient;
use crate::proto::v1::{
    BackendReport, BackendSpec, DeregisterResponse, EnrollRequest, RegisterRequest,
    RegisterResponse, RenewCertificateRequest, ReportRequest, ReportResponse, SessionRef,
};

/// The certificate file name in the state directory.
pub const CLIENT_CERT_FILE: &str = "client_cert.pem";
/// The private key file name in the state directory.
pub const CLIENT_KEY_FILE: &str = "client_key.pem";
/// The registration authority certificate file name in the state directory.
pub const CA_FILE: &str = "ca.pem";

/// The registration calls that the reconcile loop makes.
#[async_trait]
pub trait RegistrationApi: Send + Sync {
    /// Register or update backends.
    ///
    /// # Errors
    /// [`RegistrarError`] for a failed call or a superseded session.
    async fn register(&self, specs: &[BackendSpec]) -> Result<RegisterResponse, RegistrarError>;

    /// Report the health and metrics of this session's backends.
    ///
    /// # Errors
    /// [`RegistrarError`] for a failed call or a superseded session.
    async fn report(&self, reports: &[BackendReport]) -> Result<ReportResponse, RegistrarError>;

    /// Deregister backends that left the manifest.
    ///
    /// # Errors
    /// [`RegistrarError`] for a failed call or a superseded session.
    async fn deregister(&self, ids: &[String]) -> Result<DeregisterResponse, RegistrarError>;

    /// Renew the client certificate when two thirds of its life passed.
    ///
    /// # Errors
    /// [`RegistrarError`] when the renewal call fails.
    async fn renew_if_due(&self) -> Result<(), RegistrarError> {
        Ok(())
    }
}

/// The certificate, key and authority that the sidecar holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredIdentity {
    /// The client certificate in Privacy-Enhanced Mail (PEM) form.
    pub cert_pem: String,
    /// The private key in PEM form.
    pub key_pem: String,
    /// The registration authority certificate in PEM form.
    pub ca_pem: String,
}

/// Reads and writes the identity files in `state_dir`.
#[derive(Debug, Clone)]
pub struct IdentityStore {
    dir: PathBuf,
}

impl IdentityStore {
    /// A store over `dir`.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The client certificate path.
    #[must_use]
    pub fn cert_path(&self) -> PathBuf {
        self.dir.join(CLIENT_CERT_FILE)
    }

    /// The private key path.
    #[must_use]
    pub fn key_path(&self) -> PathBuf {
        self.dir.join(CLIENT_KEY_FILE)
    }

    /// The registration authority certificate path.
    #[must_use]
    pub fn ca_path(&self) -> PathBuf {
        self.dir.join(CA_FILE)
    }

    /// The stored identity, when all three files exist.
    #[must_use]
    pub fn load(&self) -> Option<StoredIdentity> {
        let cert_pem = std::fs::read_to_string(self.cert_path()).ok()?;
        let key_pem = std::fs::read_to_string(self.key_path()).ok()?;
        let ca_pem = std::fs::read_to_string(self.ca_path()).ok()?;
        Some(StoredIdentity {
            cert_pem,
            key_pem,
            ca_pem,
        })
    }

    /// Write the client certificate and key, keeping the authority.
    ///
    /// # Errors
    /// [`RegistrarError::Enroll`] when the directory or a file cannot be
    /// written.
    pub fn store_cert_and_key(&self, cert_pem: &str, key_pem: &str) -> Result<(), RegistrarError> {
        write_secret(&self.cert_path(), cert_pem.as_bytes())?;
        write_secret(&self.key_path(), key_pem.as_bytes())
    }

    /// Write all three files.
    ///
    /// # Errors
    /// [`RegistrarError::Enroll`] when the directory or a file cannot be
    /// written.
    pub fn store(&self, identity: &StoredIdentity) -> Result<(), RegistrarError> {
        write_secret(&self.cert_path(), identity.cert_pem.as_bytes())?;
        write_secret(&self.key_path(), identity.key_pem.as_bytes())?;
        write_secret(&self.ca_path(), identity.ca_pem.as_bytes())
    }

    /// The certificate's validity window in Unix seconds.
    #[must_use]
    pub fn validity(&self) -> Option<(u64, u64)> {
        let cert_pem = std::fs::read_to_string(self.cert_path()).ok()?;
        certificate_validity(&cert_pem)
    }

    /// True when two thirds of the certificate's life passed.
    #[must_use]
    pub fn renewal_due(&self, now_unix_ms: u64) -> bool {
        let Some((not_before, not_after)) = self.validity() else {
            return false;
        };
        let start = not_before.saturating_mul(1_000);
        let end = not_after.saturating_mul(1_000);
        let life = end.saturating_sub(start);
        now_unix_ms >= start.saturating_add(life.saturating_mul(2) / 3)
    }

    /// True when the certificate's `not_after` is in the past.
    #[must_use]
    pub fn is_expired(&self, now_unix_ms: u64) -> bool {
        self.validity()
            .is_some_and(|(_, not_after)| now_unix_ms >= not_after.saturating_mul(1_000))
    }
}

/// Write one secret file with mode 0600 (Unix).
fn write_secret(path: &Path, bytes: &[u8]) -> Result<(), RegistrarError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            RegistrarError::Enroll(format!("cannot make {}: {error}", parent.display()))
        })?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| {
                RegistrarError::Enroll(format!("cannot write {}: {error}", path.display()))
            })?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                RegistrarError::Enroll(format!(
                    "cannot set mode 0600 on {}: {error}",
                    path.display()
                ))
            })?;
        file.write_all(bytes).map_err(|error| {
            RegistrarError::Enroll(format!("cannot write {}: {error}", path.display()))
        })
    }
    #[cfg(not(unix))]
    {
        // Key protection at rest is spec 83 feature 08; Windows keeps its
        // inherited access control entries until then.
        let mut file = std::fs::File::create(path).map_err(|error| {
            RegistrarError::Enroll(format!("cannot write {}: {error}", path.display()))
        })?;
        file.write_all(bytes).map_err(|error| {
            RegistrarError::Enroll(format!("cannot write {}: {error}", path.display()))
        })
    }
}

/// The generated registration client plus this sidecar's identity.
pub struct RegistrationClient {
    config: RegistrarConfig,
    ctx: Ctx,
    identity: IdentityStore,
    transport: RwLock<Arc<dyn Transport>>,
    base_url: String,
    session: SessionRef,
}

impl std::fmt::Debug for RegistrationClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistrationClient")
            .field("base_url", &self.base_url)
            .field("host_id", &self.session.host_id)
            .finish_non_exhaustive()
    }
}

impl RegistrationClient {
    /// Enroll when there is no valid certificate, then connect with mTLS.
    ///
    /// A Unix-socket `proxy` URL skips enrollment: the file permissions stand
    /// in for the certificate.
    ///
    /// # Errors
    /// [`RegistrarError::Config`] for an unusable URL, and
    /// [`RegistrarError::Enroll`] for a failed enrollment or a certificate
    /// that needs a new token.
    pub async fn connect(config: &RegistrarConfig, ctx: &Ctx) -> Result<Self, RegistrarError> {
        let identity = IdentityStore::new(&config.state_dir);
        let session = SessionRef {
            session_id: random_hex(ctx, 16),
            host_id: config.host_id.clone(),
            ..Default::default()
        };
        if let Some(path) = config.proxy.strip_prefix("unix://") {
            #[cfg(unix)]
            {
                if path.is_empty() {
                    return Err(RegistrarError::Config(
                        "the unix proxy URL has no socket path".to_string(),
                    ));
                }
                let transport = unix_transport(path);
                return Ok(Self {
                    config: config.clone(),
                    ctx: ctx.clone(),
                    identity,
                    transport: RwLock::new(transport),
                    base_url: "http://localhost".to_string(),
                    session,
                });
            }
            #[cfg(not(unix))]
            return Err(RegistrarError::Config(
                "the Unix socket transport is not available on this operating system".to_string(),
            ));
        }

        let now_unix_ms = now_unix_ms(ctx);
        let stored = identity.load();
        let needs_enrollment = match &stored {
            None => true,
            Some(_) => identity.is_expired(now_unix_ms),
        };
        if needs_enrollment {
            let has_token = config
                .enrollment_token_path
                .as_ref()
                .is_some_and(|path| path.is_file());
            if !has_token && stored.is_some() {
                return Err(RegistrarError::Enroll(
                    "the client certificate expired and the enrollment token is gone; \
                     an operator must give a new enrollment token"
                        .to_string(),
                ));
            }
            enroll(config, ctx, &identity).await?;
        }
        let transport = mutual_transport(&identity, ctx)?;
        Ok(Self {
            config: config.clone(),
            ctx: ctx.clone(),
            identity,
            transport: RwLock::new(transport),
            base_url: config.proxy.clone(),
            session,
        })
    }

    /// The session identity of this process.
    #[must_use]
    pub fn session(&self) -> &SessionRef {
        &self.session
    }

    /// The identity store.
    #[must_use]
    pub fn identity(&self) -> &IdentityStore {
        &self.identity
    }

    fn client(&self) -> Result<RegistrationServiceClient, RegistrarError> {
        let transport = Arc::clone(&self.transport.read().expect("registration transport lock"));
        RegistrationServiceClient::new(transport, &self.base_url, ClientOptions::new()).map_err(
            |error| RegistrarError::Call {
                code: error.current_context().code().as_str().to_string(),
                message: error.current_context().message().to_string(),
            },
        )
    }

    fn session_field(&self) -> buffa::MessageField<SessionRef, buffa::Inline<SessionRef>> {
        Some(self.session.clone()).into()
    }

    fn map_call(&self, error: &foundation_errstacks::ErrorTrace<ConnectError>) -> RegistrarError {
        let context = error.current_context();
        match context.code() {
            Code::Aborted => RegistrarError::Superseded(self.session.host_id.clone()),
            code => RegistrarError::Call {
                code: code.as_str().to_string(),
                message: context.message().to_string(),
            },
        }
    }

    async fn within<T, F>(&self, future: F) -> Result<T, RegistrarError>
    where
        F: std::future::Future<Output = T>,
    {
        let timer = sleep_async(self.config.call_timeout, &self.ctx).boxed();
        match select(Box::pin(future), timer).await {
            Either::Left((value, _)) => Ok(value),
            Either::Right(_) => Err(RegistrarError::Call {
                code: "DEADLINE_EXCEEDED".to_string(),
                message: format!(
                    "the registration call did not answer within {:?}",
                    self.config.call_timeout
                ),
            }),
        }
    }
}

#[async_trait]
impl RegistrationApi for RegistrationClient {
    async fn register(&self, specs: &[BackendSpec]) -> Result<RegisterResponse, RegistrarError> {
        if specs.is_empty() {
            return Ok(RegisterResponse::default());
        }
        let client = self.client()?;
        let request = RegisterRequest {
            session: self.session_field(),
            backends: specs.to_vec(),
            ..Default::default()
        };
        let call = client.register(RpcCtx::background(), Request::new(request));
        let result = self.within(call).await?;
        result
            .map(|response| response.msg)
            .map_err(|error| self.map_call(&error))
    }

    async fn report(&self, reports: &[BackendReport]) -> Result<ReportResponse, RegistrarError> {
        let client = self.client()?;
        let request = ReportRequest {
            session: self.session_field(),
            backends: reports.to_vec(),
            ..Default::default()
        };
        let call = client.report(RpcCtx::background(), Request::new(request));
        let result = self.within(call).await?;
        result
            .map(|response| response.msg)
            .map_err(|error| self.map_call(&error))
    }

    async fn deregister(&self, ids: &[String]) -> Result<DeregisterResponse, RegistrarError> {
        if ids.is_empty() {
            return Ok(DeregisterResponse::default());
        }
        let client = self.client()?;
        let request = crate::proto::v1::DeregisterRequest {
            session: self.session_field(),
            backend_ids: ids.to_vec(),
            ..Default::default()
        };
        let call = client.deregister(RpcCtx::background(), Request::new(request));
        let result = self.within(call).await?;
        result
            .map(|response| response.msg)
            .map_err(|error| self.map_call(&error))
    }

    async fn renew_if_due(&self) -> Result<(), RegistrarError> {
        if !self.identity.renewal_due(now_unix_ms(&self.ctx)) {
            return Ok(());
        }
        let (key_pem, csr_pem) = pki::generate_csr(&self.session.host_id)
            .map_err(|error| RegistrarError::Enroll(error.to_string()))?;
        let client = self.client()?;
        let request = RenewCertificateRequest {
            csr_pem,
            ..Default::default()
        };
        let call = client.renew_certificate(RpcCtx::background(), Request::new(request));
        let result = self.within(call).await?;
        let response = result
            .map(|response| response.msg)
            .map_err(|error| self.map_call(&error))?;
        self.identity
            .store_cert_and_key(&response.client_cert_pem, &key_pem)?;
        let transport = mutual_transport(&self.identity, &self.ctx)?;
        *self.transport.write().expect("registration transport lock") = transport;
        tracing::info!("client certificate renewed");
        Ok(())
    }
}

/// Enroll once with the token file.
async fn enroll(
    config: &RegistrarConfig,
    ctx: &Ctx,
    identity: &IdentityStore,
) -> Result<(), RegistrarError> {
    let enrollment_url = config.enrollment_url.as_deref().ok_or_else(|| {
        RegistrarError::Config(
            "enrollment_url is required when there is no valid certificate".to_string(),
        )
    })?;
    let token_path = config.enrollment_token_path.as_ref().ok_or_else(|| {
        RegistrarError::Enroll(
            "no client certificate and no enrollment token file; an operator must give a token"
                .to_string(),
        )
    })?;
    let token = std::fs::read_to_string(token_path)
        .map_err(|error| {
            RegistrarError::Enroll(format!("cannot read {}: {error}", token_path.display()))
        })?
        .trim()
        .to_string();
    if token.is_empty() {
        return Err(RegistrarError::Enroll(format!(
            "{} is empty",
            token_path.display()
        )));
    }
    let (key_pem, csr_pem) = pki::generate_csr(&config.host_id)
        .map_err(|error| RegistrarError::Enroll(error.to_string()))?;
    let nonce = random_bytes(ctx, 32);
    let tls = client_config_enrollment(config.proxy_fingerprint_sha256.as_deref(), ctx)
        .map_err(|error| RegistrarError::Enroll(error.to_string()))?;
    let transport = tls_transport(tls);
    let client =
        RegistrarEnrollmentServiceClient::new(transport, enrollment_url, ClientOptions::new())
            .map_err(|error| {
                RegistrarError::Enroll(format!(
                    "enrollment client: {}",
                    error.current_context().message()
                ))
            })?;
    let request = EnrollRequest {
        token: token.clone(),
        host_id: config.host_id.clone(),
        csr_pem: csr_pem.clone(),
        nonce: nonce.clone(),
        ..Default::default()
    };
    let call = client.enroll(RpcCtx::background(), Request::new(request));
    let reply = select(
        Box::pin(call),
        sleep_async(config.call_timeout, ctx).boxed(),
    )
    .await;
    let response = match reply {
        Either::Left((result, _)) => result.map(|response| response.msg).map_err(|error| {
            RegistrarError::Enroll(error.current_context().message().to_string())
        })?,
        Either::Right(_) => {
            return Err(RegistrarError::Enroll(
                "the enrollment call did not answer in time".to_string(),
            ))
        }
    };

    // The server proof proves that the right proxy answered (spec 81 auth.md
    // section 9.1.1). The token key signs it; the client key is the SHA-256 of
    // the certificate signing request (CSR) PEM text.
    let secret = SignedSecret::decode(&token, &DecodeLimits::default())
        .map_err(|error| RegistrarError::Enroll(format!("the token does not decode: {error}")))?;
    let csr_hash = Sha256::digest(csr_pem.as_bytes());
    let verified = secret
        .verifying_key()
        .verify_enroll_proof(
            &secret.id(),
            &nonce,
            &csr_hash,
            &config.host_id,
            &response.server_proof,
        )
        .map_err(|error| RegistrarError::Enroll(format!("the server proof: {error}")))?;
    if !verified {
        return Err(RegistrarError::Enroll(
            "the server proof does not match the token".to_string(),
        ));
    }
    if response.client_cert_pem.is_empty() || response.ca_cert_pem.is_empty() {
        return Err(RegistrarError::Enroll(
            "the proxy reply holds no certificate or no authority".to_string(),
        ));
    }
    identity.store(&StoredIdentity {
        cert_pem: response.client_cert_pem,
        key_pem: key_pem.to_string(),
        ca_pem: response.ca_cert_pem,
    })?;
    match std::fs::remove_file(token_path) {
        Ok(()) => {}
        Err(error) => tracing::warn!(
            path = %token_path.display(),
            %error,
            "could not delete the spent enrollment token file"
        ),
    }
    tracing::info!("enrolled with the registration authority");
    Ok(())
}

/// A mutual-TLS transport from the stored identity.
fn mutual_transport(
    identity: &IdentityStore,
    ctx: &Ctx,
) -> Result<Arc<dyn Transport>, RegistrarError> {
    let stored = identity
        .load()
        .ok_or_else(|| RegistrarError::Enroll("the stored identity is incomplete".to_string()))?;
    let config = client_config_mutual(&stored.ca_pem, &stored.cert_pem, &stored.key_pem, ctx)
        .map_err(|error| RegistrarError::Enroll(error.to_string()))?;
    Ok(tls_transport(config))
}

/// A transport over a Unix socket.
#[cfg(unix)]
fn unix_transport(path: &str) -> Arc<dyn Transport> {
    let client = HttpClientBuilder::new().unix_socket(path).build();
    Arc::new(H1Transport::new(client))
}

/// A TLS transport from a `rustls` client configuration.
fn tls_transport(config: Arc<rustls::ClientConfig>) -> Arc<dyn Transport> {
    let connector = SSLConnector::with_config(config);
    let client = HttpClientBuilder::new()
        .with_tls_connector(connector)
        .build();
    Arc::new(H1Transport::new(client))
}

/// `n` random bytes from the `Ctx` generator.
fn random_bytes(ctx: &Ctx, n: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; n];
    ctx.rng()
        .lock()
        .expect("ctx random generator")
        .fill_bytes(&mut bytes);
    bytes
}

/// `n` random bytes as lowercase hexadecimal.
fn random_hex(ctx: &Ctx, n: usize) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(n * 2);
    for byte in random_bytes(ctx, n) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The wall-clock time in Unix milliseconds.
fn now_unix_ms(ctx: &Ctx) -> u64 {
    u64::try_from(foundation_compact::time::unix_millis(
        ctx.clock().wall_now(),
    ))
    .unwrap_or(0)
}

/// The `(not_before, not_after)` of a PEM certificate, in Unix seconds.
fn certificate_validity(cert_pem: &str) -> Option<(u64, u64)> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    let start = cert_pem.find(BEGIN)? + BEGIN.len();
    let end = cert_pem.find(END)?;
    let body: String = cert_pem[start..end]
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let der = base64::engine::general_purpose::STANDARD
        .decode(body)
        .ok()?;
    let (_, certificate) = x509_parser::parse_x509_certificate(&der).ok()?;
    let validity = certificate.validity();
    let not_before = u64::try_from(validity.not_before.timestamp()).ok()?;
    let not_after = u64::try_from(validity.not_after.timestamp()).ok()?;
    Some((not_before, not_after))
}
