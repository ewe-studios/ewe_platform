//! Enrollment: a signed secret and a credential become a client identity.
//!
//! WHY: a client starts with one signed secret from the operator. Enrollment
//! turns it into either a client certificate (mutual Transport Layer Security,
//! mTLS, native hosts) or a client ID plus a short access secret (every host,
//! a browser included).
//!
//! WHAT: [`ClientCertIssuer`] (the certificate authority seam),
//! [`EnrollmentService`] (`Enroll` and `RefreshAccess`), and [`EnrollConfig`]
//! limits.
//!
//! HOW: the service checks the secret, then the credential, and only then
//! spends a use. It records the client, signs the server proof last, and
//! returns the credential. A refresh needs the client's private key, so a
//! stolen access secret cannot renew itself.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foundation_auth::fingerprint::sha256;
use foundation_auth::secret::{
    ClientPublicKey, SecretError, SecretIssuer, SecretRequest, SecretVerifier, VerifiedSecret,
};
use foundation_auth::trust::{ClientKeyRecord, TrustError, TrustedClient, TrustedClients};
use foundation_compact::time::unix_millis;
use foundation_compact::Instant;
use foundation_connectrpc::{ConnectResult, Request, Response, RpcCtx};
use foundation_context::env::Env;
use foundation_context::Ctx;

use crate::core::config::EnrollConfig;
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1::bootstrap_service::BootstrapService;
use crate::proto::v1::{
    ClientKey, EnrollRequest, EnrollResponse, RefreshAccessRequest, RefreshAccessResponse,
};

/// The refresh signature domain: `"ewe.agent_rpc.refresh.v1" 0x00`.
const REFRESH_DOMAIN: &[u8] = b"ewe.agent_rpc.refresh.v1\0";

/// The certificate signing seam. Only a native host has a certificate
/// authority (CA).
pub trait ClientCertIssuer: Send + Sync + 'static {
    /// Sign one certificate signing request (CSR).
    ///
    /// # Errors
    /// [`AgentRpcError::Auth`] for an invalid CSR or client name;
    /// [`AgentRpcError::Config`] for a CA failure.
    fn sign_client_csr(
        &self,
        csr_pem: &str,
        client_name: &str,
        lifetime: foundation_compact::Duration,
    ) -> AgentRpcResult<IssuedClientCert>;

    /// The CA certificate in Privacy-Enhanced Mail (PEM).
    fn ca_cert_pem(&self) -> &str;
}

/// One issued client certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedClientCert {
    /// The certificate in PEM.
    pub cert_pem: String,
    /// The lowercase hexadecimal SHA-256 of the certificate.
    pub fingerprint_sha256: String,
    /// When the certificate expires (Unix seconds).
    pub not_after: u64,
}

/// The `BootstrapService` of one host.
pub struct EnrollmentService {
    verifier: SecretVerifier,
    signer: SecretIssuer,
    issuer: Option<Arc<dyn ClientCertIssuer>>,
    trust: Arc<TrustedClients>,
    env: Env,
    ctx: Ctx,
    config: EnrollConfig,
    rate: Mutex<HashMap<String, Bucket>>,
}

impl std::fmt::Debug for EnrollmentService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnrollmentService")
            .field("has_ca", &self.issuer.is_some())
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

impl EnrollmentService {
    /// Serve enrollment and refresh with `verifier`, `signer` and `trust`.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when the context holds no `Env`.
    pub fn new(
        verifier: SecretVerifier,
        signer: SecretIssuer,
        issuer: Option<Arc<dyn ClientCertIssuer>>,
        trust: Arc<TrustedClients>,
        ctx: Ctx,
        config: EnrollConfig,
    ) -> AgentRpcResult<Self> {
        let env = ctx
            .require_env()
            .map_err(|error| AgentRpcError::Config(format!("the context holds no Env: {error}")))?;
        Ok(Self {
            verifier,
            signer,
            issuer,
            trust,
            env: env.as_ref().clone(),
            ctx,
            config,
            rate: Mutex::new(HashMap::new()),
        })
    }

    /// The audience (the host URL) that this service enrolls for.
    #[must_use]
    pub fn audience(&self) -> Option<&str> {
        self.verifier.audience()
    }

    /// Verify the secret, check the credential, spend one use, record the
    /// client, and return the credential with a server proof.
    ///
    /// # Errors
    /// [`AgentRpcError::Auth`] for a bad, expired or spent secret, or a
    /// revoked client; [`AgentRpcError::InvalidArgument`] for a missing or
    /// malformed credential; [`AgentRpcError::Unsupported`] for a certificate
    /// request on a host with no CA; [`AgentRpcError::Storage`] for a store
    /// failure.
    pub async fn enroll_client(&self, req: &EnrollRequest) -> AgentRpcResult<EnrollResponse> {
        validate_client_name(&req.client_name, self.config.max_client_name_bytes)?;
        let nonce = check_nonce(&req.nonce)?;
        let verified = self
            .verifier
            .verify(&req.token)
            .map_err(|e| secret_error(&e))?;
        self.trust
            .check_secret_not_revoked(&verified.id)
            .await
            .map_err(|e| trust_error(&e))?;

        let client_id = self.env.ids().next_id().to_string();
        let enrolled_at = now_secs(&self.ctx);
        match req.credential.as_ref().ok_or_else(|| {
            AgentRpcError::InvalidArgument(
                "a credential (client_key or csr_pem) is required".to_string(),
            )
        })? {
            crate::proto::v1::__buffa::oneof::enroll_request::Credential::ClientKey(key) => {
                self.enroll_client_key(req, key, &verified, nonce, &client_id, enrolled_at)
                    .await
            }
            crate::proto::v1::__buffa::oneof::enroll_request::Credential::CsrPem(csr) => {
                self.enroll_client_cert(req, csr, &verified, nonce, &client_id, enrolled_at)
                    .await
            }
        }
    }

    async fn enroll_client_key(
        &self,
        req: &EnrollRequest,
        key: &ClientKey,
        verified: &VerifiedSecret,
        nonce: &[u8],
        client_id: &str,
        enrolled_at: u64,
    ) -> AgentRpcResult<EnrollResponse> {
        let key = client_key(key)?;
        let audience = self.verifier.audience().ok_or_else(|| {
            AgentRpcError::Auth("the verifier sets no audience for access secrets".to_string())
        })?;
        // Build the access secret before spending the secret, so a signing
        // failure spends nothing.
        let access_secret = self
            .signer
            .issue(&SecretRequest::access_for(
                self.config.access_lifetime,
                audience,
                client_id,
            ))
            .map_err(|e| secret_error(&e))?;
        let record = TrustedClient {
            fingerprint_sha256: key.fingerprint(),
            client_id: Some(client_id.to_string()),
            client_name: req.client_name.clone(),
            enrolled_at,
            expires_at: enrolled_at.saturating_add(self.config.client_cert_lifetime.as_secs()),
            last_seen_at: None,
            revoked_at: None,
        };
        self.spend_and_record(verified, record, "").await?;
        self.trust
            .record_client_key(
                client_id,
                &ClientKeyRecord {
                    alg: key.algorithm().as_str().to_string(),
                    public_key: key.as_bytes().to_vec(),
                },
            )
            .await
            .map_err(|e| trust_error(&e))?;
        let proof = self.server_proof(verified, nonce, key.as_bytes(), client_id)?;
        Ok(EnrollResponse {
            client_id: client_id.to_string(),
            access_secret: access_secret.encode().to_string(),
            server_proof: proof,
            ..Default::default()
        })
    }

    async fn enroll_client_cert(
        &self,
        req: &EnrollRequest,
        csr: &str,
        verified: &VerifiedSecret,
        nonce: &[u8],
        client_id: &str,
        enrolled_at: u64,
    ) -> AgentRpcResult<EnrollResponse> {
        if csr.len() > self.config.max_csr_bytes {
            return Err(AgentRpcError::InvalidArgument(format!(
                "the certificate signing request is {} bytes; the limit is {}",
                csr.len(),
                self.config.max_csr_bytes
            ))
            .into());
        }
        let issuer = self.issuer.as_ref().ok_or_else(|| {
            AgentRpcError::Unsupported("client certificates on this host".to_string())
        })?;
        let cert =
            issuer.sign_client_csr(csr, &req.client_name, self.config.client_cert_lifetime)?;
        let record = TrustedClient {
            fingerprint_sha256: cert.fingerprint_sha256.clone(),
            client_id: Some(client_id.to_string()),
            client_name: req.client_name.clone(),
            enrolled_at,
            expires_at: cert.not_after,
            last_seen_at: None,
            revoked_at: None,
        };
        self.spend_and_record(verified, record, &cert.cert_pem)
            .await?;
        let proof_key = sha256(csr.as_bytes());
        let proof = self.server_proof(verified, nonce, &proof_key, client_id)?;
        Ok(EnrollResponse {
            client_cert_pem: cert.cert_pem,
            ca_cert_pem: issuer.ca_cert_pem().to_string(),
            client_id: client_id.to_string(),
            server_proof: proof,
            ..Default::default()
        })
    }

    async fn spend_and_record(
        &self,
        verified: &VerifiedSecret,
        record: TrustedClient,
        cert_pem: &str,
    ) -> AgentRpcResult<()> {
        self.trust
            .spend_secret(
                &verified.id,
                verified.valid_until,
                self.config.max_enrollments_per_secret,
            )
            .await
            .map_err(|e| trust_error(&e))?;
        self.trust
            .record_enrollment(record, cert_pem)
            .await
            .map_err(|e| trust_error(&e))?;
        Ok(())
    }

    /// A new access secret for an enrolled client-key client.
    ///
    /// # Errors
    /// [`AgentRpcError::Auth`] for an unknown or revoked client, a bad
    /// signature, a time outside the allowed skew, or a replayed nonce;
    /// [`AgentRpcError::Storage`] for a store failure.
    pub async fn refresh_access(
        &self,
        req: &RefreshAccessRequest,
    ) -> AgentRpcResult<RefreshAccessResponse> {
        let client = self
            .trust
            .client_by_id(&req.client_id)
            .await
            .map_err(|e| trust_error(&e))?
            .ok_or_else(|| AgentRpcError::Auth("unknown client".to_string()))?;
        if client.revoked_at.is_some() {
            return Err(AgentRpcError::Auth("client revoked".to_string()).into());
        }
        let key = self
            .trust
            .client_key(&req.client_id)
            .await
            .map_err(|e| trust_error(&e))?
            .ok_or_else(|| {
                AgentRpcError::Auth(
                    "this client enrolled with a certificate, not a client key".to_string(),
                )
            })?;
        check_refresh_time(req.unix_ms, now_millis(&self.ctx), self.config.clock_skew)?;
        if req.nonce.is_empty() || req.nonce.len() > self.config.max_refresh_nonce_bytes {
            return Err(AgentRpcError::InvalidArgument(format!(
                "a refresh nonce has 1 to {} bytes",
                self.config.max_refresh_nonce_bytes
            ))
            .into());
        }
        let key =
            ClientPublicKey::parse(&key.alg, &key.public_key).map_err(|e| secret_error(&e))?;
        let message = refresh_message(&req.client_id, req.unix_ms, &req.nonce);
        key.verify(&message, &req.signature)
            .map_err(|_| AgentRpcError::Auth("the refresh signature is not valid".to_string()))?;
        if !self
            .trust
            .consume_refresh_nonce(&req.client_id, &req.nonce)
            .await
            .map_err(|e| trust_error(&e))?
        {
            return Err(AgentRpcError::Auth("refresh nonce already used".to_string()).into());
        }
        let audience = self.verifier.audience().ok_or_else(|| {
            AgentRpcError::Auth("the verifier sets no audience for access secrets".to_string())
        })?;
        let access = self
            .signer
            .issue(&SecretRequest::access_for(
                self.config.access_lifetime,
                audience,
                &req.client_id,
            ))
            .map_err(|e| secret_error(&e))?;
        Ok(RefreshAccessResponse {
            access_secret: access.encode().to_string(),
            ..Default::default()
        })
    }

    fn server_proof(
        &self,
        verified: &VerifiedSecret,
        nonce: &[u8],
        client_key: &[u8],
        client_id: &str,
    ) -> AgentRpcResult<Vec<u8>> {
        self.signer
            .sign_enroll_proof(&verified.id, nonce, client_key, client_id)
            .map_err(|e| secret_error(&e))
    }

    /// Spend one token of the per-peer rate limit.
    fn check_rate(&self, peer: &str) -> AgentRpcResult<()> {
        let now = self.ctx.clock().now();
        let mut buckets = self
            .rate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let bucket = buckets.entry(peer.to_string()).or_insert(Bucket {
            tokens: f64::from(self.config.rate.burst),
            last: now,
        });
        let elapsed = now.saturating_duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * f64::from(self.config.rate.per_second))
            .min(f64::from(self.config.rate.burst));
        bucket.last = now;
        if bucket.tokens < 1.0 {
            return Err(AgentRpcError::RateLimited("enrollment".to_string()).into());
        }
        bucket.tokens -= 1.0;
        Ok(())
    }
}

impl BootstrapService for EnrollmentService {
    async fn enroll(
        &self,
        ctx: RpcCtx,
        request: Request<EnrollRequest>,
    ) -> ConnectResult<Response<EnrollResponse>> {
        self.check_rate(&ctx.request.peer.addr)
            .map_err(|error| trace_error(&error))?;
        self.enroll_client(&request.msg)
            .await
            .map(Response::new)
            .map_err(|error| error.current_context().clone().into())
    }

    async fn refresh_access(
        &self,
        ctx: RpcCtx,
        request: Request<RefreshAccessRequest>,
    ) -> ConnectResult<Response<RefreshAccessResponse>> {
        self.check_rate(&ctx.request.peer.addr)
            .map_err(|error| trace_error(&error))?;
        self.refresh_access(&request.msg)
            .await
            .map(Response::new)
            .map_err(|error| error.current_context().clone().into())
    }
}

fn trace_error(
    error: &foundation_errstacks::ErrorTrace<AgentRpcError>,
) -> foundation_errstacks::ErrorTrace<foundation_connectrpc::ConnectError> {
    error.current_context().clone().into()
}

fn client_key(key: &ClientKey) -> AgentRpcResult<ClientPublicKey> {
    ClientPublicKey::parse(&key.alg, &key.public_key).map_err(|e| secret_error(&e))
}

fn validate_client_name(name: &str, max_bytes: usize) -> AgentRpcResult<()> {
    if name.is_empty() {
        return Err(AgentRpcError::InvalidArgument("the client name is empty".to_string()).into());
    }
    if name.len() > max_bytes {
        return Err(AgentRpcError::InvalidArgument(format!(
            "the client name is longer than {max_bytes} bytes"
        ))
        .into());
    }
    if name.chars().any(char::is_control) {
        return Err(AgentRpcError::InvalidArgument(
            "the client name has a control character".to_string(),
        )
        .into());
    }
    Ok(())
}

fn check_nonce(nonce: &[u8]) -> AgentRpcResult<&[u8]> {
    if nonce.len() != 32 {
        return Err(AgentRpcError::InvalidArgument(format!(
            "the enrollment nonce has {} bytes; it needs 32",
            nonce.len()
        ))
        .into());
    }
    Ok(nonce)
}

/// The message that a refresh signature covers.
fn refresh_message(client_id: &str, unix_ms: u64, nonce: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(REFRESH_DOMAIN.len() + client_id.len() + 8 + nonce.len());
    message.extend_from_slice(REFRESH_DOMAIN);
    message.extend_from_slice(client_id.as_bytes());
    message.extend_from_slice(&unix_ms.to_be_bytes());
    message.extend_from_slice(nonce);
    message
}

fn check_refresh_time(
    request_ms: u64,
    server_ms: u64,
    skew: foundation_compact::Duration,
) -> AgentRpcResult<()> {
    let skew_ms = skew.as_millis();
    let difference = request_ms.abs_diff(server_ms);
    if u128::from(difference) > skew_ms {
        return Err(AgentRpcError::Auth(format!(
            "the refresh request is {difference} ms from this host's clock; the limit is {skew_ms} ms"
        ))
        .into());
    }
    Ok(())
}

fn now_millis(ctx: &Ctx) -> u64 {
    u64::try_from(unix_millis(ctx.clock().wall_now())).unwrap_or(0)
}

fn now_secs(ctx: &Ctx) -> u64 {
    now_millis(ctx) / 1000
}

fn secret_error(error: &SecretError) -> foundation_errstacks::ErrorTrace<AgentRpcError> {
    match error {
        SecretError::BadSignature
        | SecretError::UnknownKey
        | SecretError::Expired
        | SecretError::NotYetValid
        | SecretError::WrongPurpose
        | SecretError::WrongAudience => AgentRpcError::Auth(error.to_string()),
        SecretError::Entropy(_) => AgentRpcError::Config(error.to_string()),
        SecretError::Malformed(_)
        | SecretError::UnsupportedVersion(_)
        | SecretError::UnsupportedAlgorithm(_)
        | SecretError::WindowTooLong
        | SecretError::BadKey(_)
        | SecretError::BadRequest(_) => AgentRpcError::InvalidArgument(error.to_string()),
    }
    .into()
}

fn trust_error(error: &TrustError) -> foundation_errstacks::ErrorTrace<AgentRpcError> {
    match &error {
        TrustError::Storage(_) => AgentRpcError::Storage(error.to_string()),
        TrustError::Spent
        | TrustError::Duplicate(_)
        | TrustError::SecretRevoked
        | TrustError::ClientRevoked
        | TrustError::UnknownClient(_)
        | TrustError::NotAtomic => AgentRpcError::Auth(error.to_string()),
    }
    .into()
}
