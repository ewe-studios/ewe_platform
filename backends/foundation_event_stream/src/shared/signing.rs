//! Webhook signing and JSON Web Key Set (JWKS) serialisation.
//!
//! The key model is `foundation_auth`'s [`Jwk`] and [`Jwks`]. The signing primitive is the
//! pure-Rust Ed25519 backend. A host supplies the key; [`WebhookSigningKey::generate`] builds one
//! from the injected entropy source for a host that has no stored key.

use crate::shared::error::SubscriptionError;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_compact::{KeyPair, Seed, Signature};
use foundation_auth::shared::jwks::{Jwk, Jwks};
use foundation_compact::rng::Rng;
use foundation_compact::time::unix_seconds as compact_unix_seconds;
use foundation_context::Ctx;
use sha2::{Digest, Sha256};

/// One signed webhook header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookSignature {
    /// The decimal UNIX timestamp in seconds.
    pub timestamp: i64,
    /// The signing key identifier.
    pub kid: String,
    /// The unpadded base64url Ed25519 signature.
    pub signature: String,
}

impl WebhookSignature {
    /// The value of the `Webhook-Signature` header.
    #[must_use]
    pub fn header_value(&self) -> String {
        format!(
            "t={},kid={},ed25519={}",
            self.timestamp, self.kid, self.signature
        )
    }
}

/// The webhook signing key and its published JSON Web Key (JWK).
#[derive(Clone)]
pub struct WebhookSigningKey {
    key_pair: KeyPair,
    kid: String,
    jwk: Jwk,
}

impl WebhookSigningKey {
    /// Build a key from a 32-byte seed.
    ///
    /// A deterministic test uses this. A host uses [`Self::generate`] or its stored seed.
    ///
    /// # Panics
    ///
    /// Panics when the seed is all zero, because the signing backend refuses it.
    #[must_use]
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let seed = Seed::new(seed);
        let key_pair = KeyPair::from_seed(seed);
        let public = *key_pair.pk;
        let x = URL_SAFE_NO_PAD.encode(public);
        // RFC 7638 thumbprint: the required members in lexicographic order.
        let thumbprint = format!(r#"{{"crv":"Ed25519","kty":"OKP","x":"{x}"}}"#);
        let digest = Sha256::digest(thumbprint.as_bytes());
        let kid = format!("ds_{}", URL_SAFE_NO_PAD.encode(digest));
        let jwk = Jwk {
            key_id: kid.clone(),
            key_type: "OKP".to_string(),
            algorithm: "EdDSA".to_string(),
            use_: "sig".to_string(),
            curve: Some("Ed25519".to_string()),
            x: Some(public.to_vec()),
            y: None,
            modulus: None,
            exponent: None,
            pem: None,
        };
        Self { key_pair, kid, jwk }
    }

    /// Generate a key from the injected entropy source.
    #[must_use]
    pub fn generate(ctx: &Ctx) -> Self {
        let mut seed = [0_u8; 32];
        {
            let rng = ctx.rng();
            // The mutex poison type comes from the platform context mutex. The closure is
            // portable across the standard and wasm backends; the function pointer is not.
            #[expect(
                clippy::redundant_closure_for_method_calls,
                reason = "the poison type is platform specific, so the closure keeps the wasm build"
            )]
            let mut rng = rng.lock().unwrap_or_else(|error| error.into_inner());
            for chunk in seed.chunks_mut(8) {
                let value = rng.next_u64().to_le_bytes();
                chunk.copy_from_slice(&value[..chunk.len()]);
            }
        }
        if seed.iter().all(|byte| *byte == 0) {
            seed[0] = 1;
        }
        Self::from_seed(seed)
    }

    /// The active signing key identifier.
    #[must_use]
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// The raw Ed25519 public key bytes.
    #[must_use]
    pub fn public_key_bytes(&self) -> [u8; 32] {
        *self.key_pair.pk
    }

    /// The published JSON Web Key (JWK).
    #[must_use]
    pub fn jwk(&self) -> &Jwk {
        &self.jwk
    }

    /// The published JSON Web Key Set (JWKS) as text.
    ///
    /// # Errors
    ///
    /// Returns an error when the key set cannot be serialized.
    pub fn jwks_json(&self) -> Result<String, SubscriptionError> {
        Jwks {
            keys: vec![self.jwk.clone()],
            issuer: None,
        }
        .to_json()
        .map_err(|error| {
            SubscriptionError::new(
                crate::shared::error::SubscriptionErrorCode::InvalidRequest,
                format!("webhook signing key cannot be published: {error}"),
            )
        })
    }

    /// Sign a webhook body.
    ///
    /// The signature covers `<timestamp>.<raw_body>`, as protocol section 7.1 defines.
    #[must_use]
    pub fn sign(&self, timestamp: i64, body: &[u8]) -> WebhookSignature {
        let mut message = format!("{timestamp}.").into_bytes();
        message.extend_from_slice(body);
        let signature: Signature = self.key_pair.sk.sign(message, None);
        WebhookSignature {
            timestamp,
            kid: self.kid.clone(),
            signature: URL_SAFE_NO_PAD.encode(signature.as_ref()),
        }
    }

    /// Sign a webhook body with the current wall-clock time.
    #[must_use]
    pub fn sign_now(&self, ctx: &Ctx, body: &[u8]) -> WebhookSignature {
        self.sign(compact_unix_seconds(ctx.clock().wall_now()), body)
    }

    /// Verify a signature over `<timestamp>.<body>` with this key.
    ///
    /// A test uses this to prove the header. A receiver uses the published key instead.
    #[must_use]
    pub fn verify(&self, timestamp: i64, body: &[u8], signature: &str) -> bool {
        let Ok(bytes) = URL_SAFE_NO_PAD.decode(signature) else {
            return false;
        };
        let Ok(signature) = Signature::from_slice(&bytes) else {
            return false;
        };
        let mut message = format!("{timestamp}.").into_bytes();
        message.extend_from_slice(body);
        self.key_pair.pk.verify(message, &signature).is_ok()
    }
}
