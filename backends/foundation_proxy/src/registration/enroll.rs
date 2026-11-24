//! Sidecar enrollment, renewal and revocation over `foundation_auth`.
//!
//! WHY: a `Register` call changes where client traffic goes. The proxy must
//! know that the caller is a sidecar that the operator trusts (spec 82 feature
//! 00, section 5). The sidecar shows a one-time enrollment token, gets a client
//! certificate, and from then on uses mutual TLS (mTLS).
//!
//! WHAT: [`EnrollmentAuthority`] over the signed-secret issuer and verifier,
//! the registration certificate authority (CA) and the trusted-client store.
//! It issues tokens (for the control socket), enrolls a certificate signing
//! request (CSR), renews a certificate and revokes a host.
//!
//! HOW: the token is a `SignedSecret` of purpose `Enroll` in the
//! `ewe.proxy.registrar_secret` domain. The verifier checks the signature, the
//! purpose, the audience and the window. The trust store spends one use, then
//! records the client. The server proof of spec 81 `auth.md` section 9.1.1 is
//! signed with the token key, so a sidecar can prove that the right proxy
//! answered.

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::pki::{self, CertificateAuthority, PkiError};
use foundation_auth::secret::{
    SecretDomain, SecretError, SecretIssuer, SecretPurpose, SecretRequest, SecretVerifier,
    VerifyPolicy,
};
use foundation_auth::trust::{TrustError, TrustedClient, TrustedClients};
use foundation_context::Ctx;
use sha2::{Digest, Sha256};

use super::proto::v1::{EnrollRequest, EnrollResponse};

/// Why enrollment, renewal or revocation failed.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display)]
pub enum EnrollmentError {
    /// The token is malformed, foreign, tampered, expired, spent or revoked, or
    /// the request does not match the token.
    #[display("enrollment refused: {_0}")]
    Unauthenticated(String),
    /// The request is not valid (a bad certificate signing request, common
    /// name or nonce).
    #[display("invalid enrollment request: {_0}")]
    Invalid(String),
    /// The certificate authority or the trust store failed.
    #[display("registration storage or certificate authority failed: {_0}")]
    Internal(String),
}

impl std::error::Error for EnrollmentError {}

impl From<PkiError> for EnrollmentError {
    fn from(error: PkiError) -> Self {
        match error {
            PkiError::BadCsr(_) | PkiError::BadClientName(_) => Self::Invalid(error.to_string()),
            _ => Self::Internal(error.to_string()),
        }
    }
}

impl From<SecretError> for EnrollmentError {
    fn from(error: SecretError) -> Self {
        match error {
            SecretError::Malformed(_)
            | SecretError::UnsupportedVersion(_)
            | SecretError::UnsupportedAlgorithm(_)
            | SecretError::UnknownKey
            | SecretError::BadSignature
            | SecretError::WrongPurpose
            | SecretError::WrongAudience
            | SecretError::WindowTooLong
            | SecretError::NotYetValid
            | SecretError::Expired => Self::Unauthenticated(error.to_string()),
            SecretError::BadKey(_) | SecretError::BadRequest(_) | SecretError::Entropy(_) => {
                Self::Internal(error.to_string())
            }
        }
    }
}

impl From<TrustError> for EnrollmentError {
    fn from(error: TrustError) -> Self {
        match error {
            TrustError::Spent
            | TrustError::SecretRevoked
            | TrustError::ClientRevoked
            | TrustError::UnknownClient(_) => Self::Unauthenticated(error.to_string()),
            TrustError::Storage(_) | TrustError::Duplicate(_) | TrustError::NotAtomic => {
                Self::Internal(error.to_string())
            }
        }
    }
}

/// The enrollment, renewal and revocation operations of the registration
/// authority.
pub struct EnrollmentAuthority {
    issuer: Arc<SecretIssuer>,
    verifier: SecretVerifier,
    trusted: Arc<TrustedClients>,
    ca: Arc<CertificateAuthority>,
    audience: Option<String>,
    client_cert_lifetime: Duration,
    default_token_validity: Duration,
    default_token_uses: u32,
    ctx: Ctx,
}

impl std::fmt::Debug for EnrollmentAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnrollmentAuthority")
            .field("audience", &self.audience)
            .finish_non_exhaustive()
    }
}

impl EnrollmentAuthority {
    /// Assemble the authority.
    ///
    /// `issuer` signs enrollment tokens and the server proof. The verifier uses
    /// its public key only, so the two always agree.
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "the authority's parts are named one by one"
    )]
    pub fn new(
        issuer: Arc<SecretIssuer>,
        trusted: Arc<TrustedClients>,
        ca: Arc<CertificateAuthority>,
        audience: Option<String>,
        client_cert_lifetime: Duration,
        default_token_validity: Duration,
        default_token_uses: u32,
        ctx: Ctx,
    ) -> Self {
        let verifier = SecretVerifier::new(
            SecretDomain::PROXY_REGISTRATION,
            vec![issuer.signing_key().verifying_key().clone()],
            ctx.clone(),
            VerifyPolicy {
                purpose: SecretPurpose::Enroll,
                audience: audience.clone(),
                clock_skew: Duration::from_mins(5),
                max_validity: Duration::from_hours(24),
                decode: foundation_auth::secret::DecodeLimits::default(),
            },
        );
        Self {
            issuer,
            verifier,
            trusted,
            ca,
            audience,
            client_cert_lifetime,
            default_token_validity,
            default_token_uses: default_token_uses.max(1),
            ctx,
        }
    }

    /// The registration certificate authority.
    #[must_use]
    pub fn ca(&self) -> &Arc<CertificateAuthority> {
        &self.ca
    }

    /// The registration CA certificate in Privacy-Enhanced Mail (PEM) form.
    #[must_use]
    pub fn ca_cert_pem(&self) -> &str {
        self.ca.ca_cert_pem()
    }

    /// The trusted-client store.
    #[must_use]
    pub fn trusted_clients(&self) -> &Arc<TrustedClients> {
        &self.trusted
    }

    /// A new enrollment token for `host_id`, valid for `validity` and spendable
    /// `uses` times. It is bound to the host as its subject.
    ///
    /// # Errors
    /// [`EnrollmentError`] when the key cannot sign or the entropy source
    /// fails.
    pub fn issue_token(
        &self,
        host_id: &str,
        validity: Option<Duration>,
        uses: Option<u32>,
    ) -> Result<String, EnrollmentError> {
        pki::validate_client_name(host_id)?;
        let mut request = SecretRequest::enroll(validity.unwrap_or(self.default_token_validity))
            .with_max_uses(uses.unwrap_or(self.default_token_uses).max(1))
            .with_subject(host_id);
        request.audience.clone_from(&self.audience);
        let secret = self.issuer.issue(&request)?;
        Ok(secret.encode().to_string())
    }

    /// Verify the token, sign the CSR, spend one use, record the client and
    /// return the certificate with the server proof.
    ///
    /// # Errors
    /// [`EnrollmentError`] for a refused token, an invalid request or a store
    /// failure.
    pub async fn enroll(&self, request: &EnrollRequest) -> Result<EnrollResponse, EnrollmentError> {
        pki::validate_client_name(&request.host_id)?;
        if request.nonce.len() != 32 {
            return Err(EnrollmentError::Invalid(
                "the nonce must be 32 bytes".to_string(),
            ));
        }
        let verified = self.verifier.verify(&request.token)?;
        if let Some(subject) = &verified.subject {
            if subject != &request.host_id {
                return Err(EnrollmentError::Unauthenticated(
                    "the token is for another host".to_string(),
                ));
            }
        }
        self.trusted.check_secret_not_revoked(&verified.id).await?;
        // Signing first validates the CSR, so a bad request spends no use.
        let issued = self.ca.sign_client_csr(
            &request.csr_pem,
            &request.host_id,
            self.client_cert_lifetime,
        )?;
        let uses = verified.max_uses.unwrap_or(self.default_token_uses).max(1);
        self.trusted
            .spend_secret(&verified.id, verified.valid_until, uses)
            .await?;
        let now = self.now_unix();
        self.trusted
            .record_enrollment(
                TrustedClient {
                    fingerprint_sha256: issued.fingerprint_sha256.clone(),
                    client_id: None,
                    client_name: request.host_id.clone(),
                    enrolled_at: now,
                    expires_at: issued.not_after,
                    last_seen_at: None,
                    revoked_at: None,
                },
                &issued.cert_pem,
            )
            .await?;
        let csr_hash = Sha256::digest(request.csr_pem.as_bytes());
        let proof = self.issuer.sign_enroll_proof(
            &verified.id,
            &request.nonce,
            &csr_hash,
            &request.host_id,
        )?;
        Ok(EnrollResponse {
            client_cert_pem: issued.cert_pem,
            ca_cert_pem: self.ca.ca_cert_pem().to_string(),
            server_proof: proof,
            ..Default::default()
        })
    }

    /// Issue a new certificate for an already enrolled `client_name` from a
    /// fresh CSR. The caller proves the old certificate over mutual TLS.
    ///
    /// # Errors
    /// [`EnrollmentError`] for an invalid CSR or a store failure.
    pub async fn renew(&self, csr_pem: &str, client_name: &str) -> Result<String, EnrollmentError> {
        let issued = self
            .ca
            .sign_client_csr(csr_pem, client_name, self.client_cert_lifetime)?;
        let now = self.now_unix();
        self.trusted
            .record_enrollment(
                TrustedClient {
                    fingerprint_sha256: issued.fingerprint_sha256,
                    client_id: None,
                    client_name: client_name.to_string(),
                    enrolled_at: now,
                    expires_at: issued.not_after,
                    last_seen_at: None,
                    revoked_at: None,
                },
                &issued.cert_pem,
            )
            .await?;
        Ok(issued.cert_pem)
    }

    /// The enrolled client name for a certificate fingerprint, when the client
    /// is known and active.
    ///
    /// # Errors
    /// [`TrustError::Storage`] when the store cannot be read.
    pub async fn client_name_for(
        &self,
        fingerprint_sha256: &str,
    ) -> Result<Option<String>, EnrollmentError> {
        let now = self.now_unix();
        Ok(self
            .trusted
            .get(fingerprint_sha256)
            .await?
            .filter(|client| client.is_active_at(now))
            .map(|client| client.client_name))
    }

    /// Record a successful call from a client (best effort at the store's
    /// `touch_interval`).
    ///
    /// # Errors
    /// [`TrustError::Storage`] when the store fails.
    pub async fn touch(&self, fingerprint_sha256: &str) -> Result<(), EnrollmentError> {
        self.trusted.touch(fingerprint_sha256).await?;
        Ok(())
    }

    /// Revoke every active certificate of `host_id`. Returns the number
    /// revoked.
    ///
    /// # Errors
    /// [`EnrollmentError::Internal`] when the store fails.
    pub async fn revoke_host(&self, host_id: &str) -> Result<usize, EnrollmentError> {
        let clients = self.trusted.clients().await?;
        let mut revoked = 0;
        for client in clients {
            if client.client_name == host_id
                && client.revoked_at.is_none()
                && self
                    .trusted
                    .revoke_client(&client.fingerprint_sha256)
                    .await?
            {
                revoked += 1;
            }
        }
        Ok(revoked)
    }

    fn now_unix(&self) -> u64 {
        u64::try_from(foundation_compact::time::unix_seconds(
            self.ctx.clock().wall_now(),
        ))
        .unwrap_or(0)
    }
}
