//! The verifier: it holds only public keys and checks presented secrets.

use foundation_compact::Duration;
use foundation_context::Ctx;

use super::{
    codec, DecodeLimits, SecretDomain, SecretError, SecretPurpose, SignedSecret, VerifyingKey,
};

/// Checks signed secrets. Needs only public keys.
pub struct SecretVerifier {
    domain: SecretDomain,
    /// More than one key during a rotation.
    keys: Vec<VerifyingKey>,
    ctx: Ctx,
    policy: VerifyPolicy,
}

impl std::fmt::Debug for SecretVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretVerifier")
            .field("domain", &self.domain.0)
            .field("keys", &self.keys.len())
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

/// What the verifier accepts. Every limit is configuration (root rule 8).
#[derive(Debug, Clone)]
pub struct VerifyPolicy {
    /// The purpose that this verifier accepts.
    pub purpose: SecretPurpose,
    /// Required audience. `None` = accept only a secret with no audience.
    pub audience: Option<String>,
    /// Allowed clock error at both ends of the window. Default 5 minutes: the
    /// default `clockskew` of MIT Kerberos (300 s).
    pub clock_skew: Duration,
    /// The longest window accepted. Default 24 hours for `Enroll` (the kubeadm
    /// bootstrap token lifetime) and 30 days for `Access` (the longest that a
    /// bearer secret may live without renewal).
    pub max_validity: Duration,
    /// The decode limits.
    pub decode: DecodeLimits,
}

/// A secret that passed every check of [`SecretVerifier::verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedSecret {
    /// The secret ID. See [`SignedSecret::id`].
    pub id: String,
    /// What the secret may be used for.
    pub purpose: SecretPurpose,
    /// The start of the validity window (Unix seconds).
    pub valid_from: u64,
    /// The end of the validity window (Unix seconds, exclusive).
    pub valid_until: u64,
    /// The most uses that the issuer allowed.
    pub max_uses: Option<u32>,
    /// The audience.
    pub audience: Option<String>,
    /// The `client_id` that enrollment or refresh set.
    pub subject: Option<String>,
    /// The fingerprint of the public key that matched.
    pub key_fingerprint: String,
}

impl SecretVerifier {
    /// A verifier over `keys`, with the clock of `ctx`.
    #[must_use]
    pub fn new(
        domain: SecretDomain,
        keys: Vec<VerifyingKey>,
        ctx: Ctx,
        policy: VerifyPolicy,
    ) -> Self {
        Self {
            domain,
            keys,
            ctx,
            policy,
        }
    }

    /// The audience that this verifier requires.
    #[must_use]
    pub fn audience(&self) -> Option<&str> {
        self.policy.audience.as_deref()
    }

    /// Check `presented` now.
    ///
    /// The steps run in this order; the cheap checks come first:
    ///
    /// 1. size: at most `policy.decode.max_encoded_bytes`;
    /// 2. format: base64url, JSON, known version and algorithm, 32 secret
    ///    bytes, a non-empty window;
    /// 3. key: by `kid` (version 2), else by the exact public key bytes;
    /// 4. signature over the domain-separated message;
    /// 5. purpose equal to `policy.purpose` (version 1 = `Enroll`);
    /// 6. audience equal to `policy.audience` when the policy sets one;
    /// 7. window length at most `policy.max_validity`;
    /// 8. time: `valid_from - skew <= now < valid_until + skew`, with
    ///    `now = ctx.clock().wall_now()`.
    ///
    /// Use counts and revocation are the next step, in the trust store
    /// (`TrustedClients::spend_secret`, `check_secret_not_revoked`).
    ///
    /// # Errors
    /// One [`SecretError`] variant per failed step.
    pub fn verify(&self, presented: &str) -> Result<VerifiedSecret, SecretError> {
        // 1. Size.
        if presented.len() > self.policy.decode.max_encoded_bytes {
            return Err(SecretError::Malformed(format!(
                "the encoded secret is {} bytes; the limit is {}",
                presented.len(),
                self.policy.decode.max_encoded_bytes
            )));
        }

        // 2. Format.
        let secret = SignedSecret::decode(presented, &self.policy.decode)?;

        // 3. Key.
        let key = self.select_key(&secret).ok_or(SecretError::UnknownKey)?;

        // 4. Signature.
        let message = codec::signed_message(&secret, self.domain);
        if !key.verify(&message, &secret.signature)? {
            return Err(SecretError::BadSignature);
        }

        // 5. Purpose.
        if secret.purpose != self.policy.purpose {
            return Err(SecretError::WrongPurpose);
        }

        // 6. Audience.
        let audience_ok = match (&self.policy.audience, &secret.audience) {
            (Some(required), Some(presented)) => required == presented,
            (None, None) => true,
            _ => false,
        };
        if !audience_ok {
            return Err(SecretError::WrongAudience);
        }

        // 7. Window length.
        let window = secret.valid_until.saturating_sub(secret.valid_from);
        if window > self.policy.max_validity.as_secs() {
            return Err(SecretError::WindowTooLong);
        }

        // 8. Time.
        let now = self.now_unix();
        let skew = self.policy.clock_skew.as_secs();
        if now < secret.valid_from.saturating_sub(skew) {
            return Err(SecretError::NotYetValid);
        }
        if now >= secret.valid_until.saturating_add(skew) {
            return Err(SecretError::Expired);
        }

        Ok(VerifiedSecret {
            id: secret.id(),
            purpose: secret.purpose,
            valid_from: secret.valid_from,
            valid_until: secret.valid_until,
            max_uses: secret.max_uses,
            audience: secret.audience.clone(),
            subject: secret.subject.clone(),
            key_fingerprint: key.fingerprint(),
        })
    }

    fn select_key(&self, secret: &SignedSecret) -> Option<&VerifyingKey> {
        match secret.kid() {
            Some(kid) => self.keys.iter().find(|key| {
                key.algorithm() == secret.alg
                    && key.kid() == Some(kid)
                    && key.as_bytes() == secret.public_key.as_slice()
            }),
            None => self.keys.iter().find(|key| {
                key.algorithm() == secret.alg && key.as_bytes() == secret.public_key.as_slice()
            }),
        }
    }

    fn now_unix(&self) -> u64 {
        u64::try_from(foundation_compact::time::unix_seconds(
            self.ctx.clock().wall_now(),
        ))
        .unwrap_or(0)
    }
}

impl VerifyPolicy {
    /// The policy of an enrollment secret: purpose `Enroll`, no audience,
    /// 24 hours, 5 minutes of clock skew.
    #[must_use]
    pub fn enroll() -> Self {
        Self {
            purpose: SecretPurpose::Enroll,
            audience: None,
            clock_skew: Duration::from_mins(5),
            max_validity: Duration::from_hours(24),
            decode: DecodeLimits::default(),
        }
    }

    /// The policy of an access secret for one host: purpose `Access`, this
    /// audience, 30 days, 5 minutes of clock skew.
    #[must_use]
    pub fn access(audience: impl Into<String>) -> Self {
        Self {
            purpose: SecretPurpose::Access,
            audience: Some(audience.into()),
            clock_skew: Duration::from_mins(5),
            max_validity: Duration::from_hours(30 * 24),
            decode: DecodeLimits::default(),
        }
    }

    /// Set the allowed clock skew.
    #[must_use]
    pub fn with_clock_skew(mut self, skew: Duration) -> Self {
        self.clock_skew = skew;
        self
    }

    /// Set the longest accepted window.
    #[must_use]
    pub fn with_max_validity(mut self, max_validity: Duration) -> Self {
        self.max_validity = max_validity;
        self
    }

    /// Set the decode limits.
    #[must_use]
    pub fn with_decode_limits(mut self, decode: DecodeLimits) -> Self {
        self.decode = decode;
        self
    }
}
