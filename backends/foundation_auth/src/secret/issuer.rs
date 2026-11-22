//! The issuer: it holds the private key and makes signed secrets.

use foundation_compact::Duration;
use foundation_context::Ctx;
use zeroize::Zeroizing;

use super::{
    codec, SecretDomain, SecretError, SecretPurpose, SignedSecret, SigningAlgorithm, SigningKey,
    SECRET_BYTES, VERSION_1, VERSION_2,
};

/// Makes signed secrets. Needs the private key and the injected key material
/// sources: the clock and the entropy source.
pub struct SecretIssuer {
    domain: SecretDomain,
    key: SigningKey,
    ctx: Ctx,
    max_validity: Duration,
}

impl std::fmt::Debug for SecretIssuer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretIssuer")
            .field("domain", &self.domain.0)
            .field("key", &self.key)
            .field("max_validity_secs", &self.max_validity.as_secs())
            .finish_non_exhaustive()
    }
}

/// What to issue.
#[derive(Debug, Clone)]
pub struct SecretRequest {
    /// The purpose of the secret.
    pub purpose: SecretPurpose,
    /// How long the window is.
    pub validity: Duration,
    /// `None` = valid from now.
    pub not_before_unix: Option<u64>,
    /// The audience: the base URL of the one host that accepts it.
    pub audience: Option<String>,
    /// The most uses that the issuer allows.
    pub max_uses: Option<u32>,
    /// The `client_id` of an enrolled client, for an access secret that
    /// enrollment or refresh issued.
    pub subject: Option<String>,
}

impl SecretRequest {
    /// An enrollment secret, as today (version 1 on the wire when the key has
    /// no key ID).
    #[must_use]
    pub fn enroll(validity: Duration) -> Self {
        Self {
            purpose: SecretPurpose::Enroll,
            validity,
            not_before_unix: None,
            audience: None,
            max_uses: None,
            subject: None,
        }
    }

    /// An access secret for one host (version 2 on the wire).
    #[must_use]
    pub fn access(validity: Duration, audience: impl Into<String>) -> Self {
        Self {
            purpose: SecretPurpose::Access,
            validity,
            not_before_unix: None,
            audience: Some(audience.into()),
            max_uses: None,
            subject: None,
        }
    }

    /// An access secret that enrollment or refresh issued for `client_id`.
    #[must_use]
    pub fn access_for(
        validity: Duration,
        audience: impl Into<String>,
        client_id: impl Into<String>,
    ) -> Self {
        let mut request = Self::access(validity, audience);
        request.subject = Some(client_id.into());
        request
    }

    /// Set a use limit.
    #[must_use]
    pub fn with_max_uses(mut self, max_uses: u32) -> Self {
        self.max_uses = Some(max_uses);
        self
    }

    /// Set the start of the window (Unix seconds).
    #[must_use]
    pub fn with_not_before(mut self, unix_secs: u64) -> Self {
        self.not_before_unix = Some(unix_secs);
        self
    }

    /// Set the subject.
    #[must_use]
    pub fn with_subject(mut self, client_id: impl Into<String>) -> Self {
        self.subject = Some(client_id.into());
        self
    }
}

impl SecretIssuer {
    /// An issuer over `key`, with the clock and entropy of `ctx`.
    #[must_use]
    pub fn new(domain: SecretDomain, key: SigningKey, ctx: Ctx, max_validity: Duration) -> Self {
        Self {
            domain,
            key,
            ctx,
            max_validity,
        }
    }

    /// The domain of this issuer.
    #[must_use]
    pub fn domain(&self) -> SecretDomain {
        self.domain
    }

    /// The signing key.
    #[must_use]
    pub fn signing_key(&self) -> &SigningKey {
        &self.key
    }

    /// Make a secret.
    ///
    /// The version is 1 when the request is an enrollment secret with no
    /// audience, no use limit, no subject and the key has no key ID. That is
    /// the same secret as today, so an old client still reads it. Every other
    /// request makes version 2.
    ///
    /// # Errors
    /// [`SecretError::BadRequest`] for an empty window or a window longer than
    /// `max_validity`; [`SecretError::Entropy`] when the entropy source fails.
    pub fn issue(&self, request: &SecretRequest) -> Result<SignedSecret, SecretError> {
        let seconds = request.validity.as_secs();
        if seconds == 0 {
            return Err(SecretError::BadRequest(
                "the validity window is empty".to_string(),
            ));
        }
        if seconds > self.max_validity.as_secs() {
            return Err(SecretError::BadRequest(format!(
                "the validity window is {seconds} s; the limit is {} s",
                self.max_validity.as_secs()
            )));
        }
        let from = request.not_before_unix.unwrap_or_else(|| self.now_unix());
        let until = from.saturating_add(seconds);

        let mut secret_bytes = Zeroizing::new(vec![0u8; SECRET_BYTES]);
        self.ctx
            .require_env()
            .map_err(|e| SecretError::Entropy(e.to_string()))?
            .entropy()
            .fill(secret_bytes.as_mut_slice())
            .map_err(|e| SecretError::Entropy(e.to_string()))?;

        let kid = self.key.verifying_key().kid().map(str::to_string);
        let version = if request.purpose == SecretPurpose::Enroll
            && request.audience.is_none()
            && request.max_uses.is_none()
            && request.subject.is_none()
            && kid.is_none()
        {
            VERSION_1
        } else {
            VERSION_2
        };

        let mut secret = SignedSecret {
            version,
            alg: SigningAlgorithm::Ed25519,
            kid,
            public_key: self.key.verifying_key().as_bytes().to_vec(),
            secret: secret_bytes,
            valid_from: from,
            valid_until: until,
            purpose: request.purpose,
            audience: request.audience.clone(),
            max_uses: request.max_uses,
            subject: request.subject.clone(),
            signature: Vec::new(),
        };
        let message = codec::signed_message(&secret, self.domain);
        secret.signature = self.key.sign(&message)?;
        Ok(secret)
    }

    fn now_unix(&self) -> u64 {
        u64::try_from(foundation_compact::time::unix_seconds(
            self.ctx.clock().wall_now(),
        ))
        .unwrap_or(0)
    }
}
