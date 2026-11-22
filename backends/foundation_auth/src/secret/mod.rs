//! Signed secrets: our own base64url JSON envelope, not a JSON Web Token (JWT).
//!
//! WHY: a client starts with nothing but a short string that an operator hands
//! it out of band. That string must prove that the key holder made it, must
//! stop working after a while, and must say what it may be used for.
//!
//! WHAT: the shared types ([`SecretDomain`], [`SecretPurpose`],
//! [`SigningAlgorithm`], [`DecodeLimits`], [`SecretError`]), the decoded
//! [`SignedSecret`], and the two halves of the service: [`SecretIssuer`] signs
//! with a private key, [`SecretVerifier`] checks with public keys only.
//!
//! HOW: the signature covers a domain-separated, length-prefixed byte string,
//! never the JSON text. JSON has more than one way to write the same object, so
//! a signature over the text would change with the writer.
//!
//! Version 1 is the format of the agent RPC branch for today. A version 1
//! secret from that code still verifies unchanged. Version 2 adds the purpose,
//! the audience, the key ID, the use limit and the subject.

pub mod codec;
pub mod issuer;
pub mod keys;
pub mod verifier;

pub use issuer::{SecretIssuer, SecretRequest};
pub use keys::{SigningKey, VerifyingKey};
pub use verifier::{SecretVerifier, VerifiedSecret, VerifyPolicy};

use zeroize::Zeroizing;

/// The format version that a signed secret uses.
pub(crate) const VERSION_1: u32 = 1;
/// The additive format version: purpose, audience, key ID, use limit, subject.
pub(crate) const VERSION_2: u32 = 2;

/// The number of random bytes in a secret. A security parameter, not a limit:
/// 256 bits, as the token it replaces.
pub(crate) const SECRET_BYTES: usize = 32;

/// Separates the signatures of one use of a key from all other uses.
///
/// The signed message starts with `<domain>.v<version>` and a zero byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretDomain(pub &'static str);

impl SecretDomain {
    /// Agent RPC bootstrap secrets. Version 1 signs exactly the bytes of today.
    pub const AGENT_RPC: SecretDomain = SecretDomain("ewe.agent_rpc.bootstrap_secret");
    /// Enrollment tokens of registrar sidecars at the proxy registration
    /// authority (spec 82 feature 00, section 5). A different domain, so an
    /// agent RPC secret never enrolls a sidecar, and the other way round.
    pub const PROXY_REGISTRATION: SecretDomain = SecretDomain("ewe.proxy.registrar_secret");
}

/// What a secret may be used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretPurpose {
    /// Exchange for a client certificate (`BootstrapService/Enroll`).
    Enroll,
    /// Show as a bearer credential on each call (Workers, browsers).
    Access,
}

impl SecretPurpose {
    /// The name in the `purpose` field.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enroll => "enroll",
            Self::Access => "access",
        }
    }

    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "enroll" => Some(Self::Enroll),
            "access" => Some(Self::Access),
            _ => None,
        }
    }
}

/// The signing algorithm.
///
/// The enum is the same on every target. An algorithm that a build cannot run
/// gives [`SecretError::UnsupportedAlgorithm`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningAlgorithm {
    /// Ed25519 (RFC 8032). The default; all targets.
    Ed25519,
    /// RSA-PSS with SHA-256, 2048 to 8192 bits. Operator keys only; native
    /// with feature `secret-rsa`.
    RsaPssSha256,
}

impl SigningAlgorithm {
    /// The name in the `alg` field.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
            Self::RsaPssSha256 => "rsa-pss-sha256",
        }
    }

    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "ed25519" => Some(Self::Ed25519),
            "rsa-pss-sha256" => Some(Self::RsaPssSha256),
            _ => None,
        }
    }
}

/// A decoded signed secret.
///
/// `Debug` and `Display` never show the secret bytes.
#[derive(Clone)]
pub struct SignedSecret {
    pub(crate) version: u32,
    pub(crate) alg: SigningAlgorithm,
    pub(crate) kid: Option<String>,
    pub(crate) public_key: Vec<u8>,
    pub(crate) secret: Zeroizing<Vec<u8>>,
    pub(crate) valid_from: u64,
    pub(crate) valid_until: u64,
    pub(crate) purpose: SecretPurpose,
    pub(crate) audience: Option<String>,
    pub(crate) max_uses: Option<u32>,
    pub(crate) subject: Option<String>,
    pub(crate) signature: Vec<u8>,
}

impl std::fmt::Debug for SignedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedSecret")
            .field("version", &self.version)
            .field("alg", &self.alg)
            .field("kid", &self.kid)
            .field("valid_from", &self.valid_from)
            .field("valid_until", &self.valid_until)
            .field("purpose", &self.purpose)
            .field("audience", &self.audience)
            .field("max_uses", &self.max_uses)
            .field("has_subject", &self.subject.is_some())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for SignedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted signed secret>")
    }
}

impl SignedSecret {
    /// The shared form (base64url JSON). Treat it as a secret.
    #[must_use]
    pub fn encode(&self) -> Zeroizing<String> {
        codec::encode(self)
    }

    /// The exact bytes that the signature covers, for an audit and a golden
    /// test. The bytes start with `<domain>.v<version>` and a zero byte.
    #[must_use]
    pub fn signed_bytes(&self, domain: SecretDomain) -> Zeroizing<Vec<u8>> {
        codec::signed_message(self, domain)
    }

    /// Parse the shared form. This checks the format only.
    ///
    /// # Errors
    /// [`SecretError::Malformed`], [`SecretError::UnsupportedVersion`],
    /// [`SecretError::UnsupportedAlgorithm`].
    pub fn decode(encoded: &str, limits: &DecodeLimits) -> Result<Self, SecretError> {
        codec::decode(encoded, *limits)
    }

    /// The ID: lowercase hexadecimal SHA-256 of the 32 secret bytes. Safe to log
    /// and to store. Nobody can get the secret back from it.
    #[must_use]
    pub fn id(&self) -> String {
        crate::fingerprint::sha256_hex(&self.secret)
    }

    /// A client-side check: the signature matches the key that the secret
    /// carries, for the agent RPC domain. It finds a typing error. It does not
    /// prove who made it.
    ///
    /// A proxy registration secret uses
    /// [`verify_self_signed_for`](Self::verify_self_signed_for) with
    /// [`SecretDomain::PROXY_REGISTRATION`].
    #[must_use]
    pub fn verify_self_signed(&self) -> bool {
        self.verify_self_signed_for(SecretDomain::AGENT_RPC)
    }

    /// A client-side check for the given domain.
    #[must_use]
    pub fn verify_self_signed_for(&self, domain: SecretDomain) -> bool {
        let key = VerifyingKey {
            alg: self.alg,
            bytes: self.public_key.clone(),
            kid: self.kid.clone(),
        };
        let message = codec::signed_message(self, domain);
        key.verify(&message, &self.signature).unwrap_or(false)
    }

    /// The format version.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// The signing algorithm.
    #[must_use]
    pub fn algorithm(&self) -> SigningAlgorithm {
        self.alg
    }

    /// The key ID, when the secret has one.
    #[must_use]
    pub fn kid(&self) -> Option<&str> {
        self.kid.as_deref()
    }

    /// The raw public key bytes that the secret carries.
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// What the secret may be used for.
    #[must_use]
    pub fn purpose(&self) -> SecretPurpose {
        self.purpose
    }

    /// The audience, when the secret has one.
    #[must_use]
    pub fn audience(&self) -> Option<&str> {
        self.audience.as_deref()
    }

    /// The subject (a `client_id`) that enrollment or refresh set.
    #[must_use]
    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    /// The most uses that the issuer allows, when it set a limit.
    #[must_use]
    pub fn max_uses(&self) -> Option<u32> {
        self.max_uses
    }

    /// The start of the validity window (Unix seconds).
    #[must_use]
    pub fn valid_from(&self) -> u64 {
        self.valid_from
    }

    /// The end of the validity window (Unix seconds, exclusive).
    #[must_use]
    pub fn valid_until(&self) -> u64 {
        self.valid_until
    }
}

/// Size limits for decoding (root rule 8: limits are configuration).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Largest accepted encoded secret, in bytes. Default 8192: the size of
    /// one nginx `large_client_header_buffers` buffer (8k), because the
    /// secret travels in an HTTP header.
    pub max_encoded_bytes: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_encoded_bytes: 8192,
        }
    }
}

impl DecodeLimits {
    /// Set the largest accepted encoded secret.
    #[must_use]
    pub fn with_max_encoded_bytes(mut self, bytes: usize) -> Self {
        self.max_encoded_bytes = bytes;
        self
    }
}

/// Every way a secret can fail.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum SecretError {
    /// The text is not a signed secret.
    #[display("malformed secret: {_0}")]
    Malformed(#[error(not(source))] String),
    /// The format version is not one that this build reads.
    #[display("secret format version {_0} is not supported")]
    UnsupportedVersion(#[error(not(source))] u32),
    /// The algorithm is not one that this build can run.
    #[display("unsupported algorithm: {_0}")]
    UnsupportedAlgorithm(#[error(not(source))] String),
    /// The secret was signed by a key that the verifier does not hold.
    #[display("the secret was signed by an unknown key")]
    UnknownKey,
    /// The signature does not match.
    #[display("the secret signature is not valid")]
    BadSignature,
    /// The secret is for another purpose.
    #[display("the secret is for another purpose")]
    WrongPurpose,
    /// The secret is for another host.
    #[display("the secret is for another host")]
    WrongAudience,
    /// The validity window is longer than the policy allows.
    #[display("the secret window is longer than allowed")]
    WindowTooLong,
    /// The validity window has not started.
    #[display("the secret is not valid yet")]
    NotYetValid,
    /// The validity window has ended.
    #[display("the secret has expired")]
    Expired,
    /// A key is not in the expected form.
    #[display("bad key: {_0}")]
    BadKey(#[error(not(source))] String),
    /// The issuer got a request that it cannot honour.
    #[display("bad request: {_0}")]
    BadRequest(#[error(not(source))] String),
    /// The entropy source failed.
    #[display("entropy source failed: {_0}")]
    Entropy(#[error(not(source))] String),
}
