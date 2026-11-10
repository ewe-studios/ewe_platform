//! Multi-factor login service — TOTP secrets and one-time MFA challenges.
//!
//! A TOTP secret is stored encrypted (ChaCha20-Poly1305, key from the `IdP`
//! config, key id stored with the ciphertext, the user id as associated data).
//! The service remembers the last accepted time step per user and rejects a
//! code for the same or an earlier step, so a code works once.
//!
//! A user with a stored TOTP secret must prove it after the password. The
//! login handler then answers `200 mfa_required` with a `challenge_id`. The
//! challenge is 256 random bits, stored as its SHA-256, bound to the user and
//! the pending login, expiring after `IdpConfig::mfa_challenge_ttl`, and it
//! works once. `POST /auth/v1/mfa` verifies the one-time code against the
//! secret of the challenge's user. The body never names the user.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use zeroize::Zeroizing;

use crate::shared::clock::now_millis;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use foundation_compact::rng::Rng;
use foundation_core::valtron::Ctx;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::super::config::TotpKeyRing;
use super::super::models::PendingLogin;
use super::super::storage::{HandlerStorage, StorageOpError};
use crate::shared::two_factor::{TOTPSecret, TotpAlgorithm};
use foundation_db::KeyValueStore;

/// How many 30-second steps before and after now a TOTP code may be off.
const TOTP_TOLERANCE_STEPS: u64 = 1;

fn totp_key(user_id: &str) -> String {
    format!("mfa:totp:{user_id}")
}

/// Cache key of the last accepted TOTP time step of a user.
fn step_key(user_id: &str) -> String {
    format!("mfa:totp_step:{user_id}")
}

/// Cache key of a TOTP secret that is waiting for its first code.
fn pending_key(user_id: &str) -> String {
    format!("mfa:totp_pending:{user_id}")
}

/// A TOTP secret as stored: encrypted, with the id of the key that did it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedSecret {
    /// The id of the key in the [`TotpKeyRing`].
    pub key_id: String,
    /// The 96-bit random nonce, base64url.
    pub nonce: String,
    /// The ciphertext with its 128-bit tag, base64url.
    pub ciphertext: String,
    /// The hash of the secret. Records made before this field existed used
    /// SHA-256.
    #[serde(default = "legacy_algorithm")]
    pub algorithm: TotpAlgorithm,
}

fn legacy_algorithm() -> TotpAlgorithm {
    TotpAlgorithm::Sha256
}

/// A new TOTP secret that the user has not confirmed yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingEnrollment {
    secret: EncryptedSecret,
    /// Unix time in milliseconds after which the enrollment is not valid.
    expires_at: i64,
}

/// What the user needs to add the secret to an authenticator app.
#[derive(Debug, Clone)]
pub struct TotpEnrollment {
    /// The secret, base32.
    pub secret_base32: String,
    /// The hash of the secret (for the `algorithm` of the provisioning URI).
    pub algorithm: TotpAlgorithm,
}

fn encrypt_secret(
    ring: &TotpKeyRing,
    user_id: &str,
    secret: &TOTPSecret,
) -> Result<EncryptedSecret, StorageOpError> {
    let (key_id, key) = ring
        .active()
        .ok_or_else(|| StorageOpError::Query("no TOTP encryption key is configured".into()))?;
    // The zero array is only the buffer. The next line fills all 12 bytes from
    // the CSPRNG, so every call gets a fresh random 96-bit nonce, as
    // ChaCha20-Poly1305 requires. The nonce is stored with the ciphertext.
    // If the fill is removed or made conditional, this becomes a nonce reuse bug.
    let mut nonce_bytes = [0u8; 12];
    foundation_compact::rng::rng().fill_bytes(&mut nonce_bytes);
    let plaintext = Zeroizing::new(secret.to_base32().into_bytes());
    let ciphertext = ChaCha20Poly1305::new(Key::from_slice(key))
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: &plaintext,
                aad: user_id.as_bytes(),
            },
        )
        .map_err(|_| StorageOpError::Query("TOTP secret encryption failed".into()))?;
    Ok(EncryptedSecret {
        key_id: key_id.to_string(),
        nonce: URL_SAFE_NO_PAD.encode(nonce_bytes),
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
        algorithm: secret.algorithm(),
    })
}

fn decrypt_secret(
    ring: &TotpKeyRing,
    user_id: &str,
    stored: &EncryptedSecret,
) -> Result<TOTPSecret, StorageOpError> {
    let parse = |what: &str| StorageOpError::Parse(format!("stored TOTP secret: {what}"));
    let key = ring
        .get(&stored.key_id)
        .ok_or_else(|| parse("its encryption key is not in the key ring"))?;
    let nonce = URL_SAFE_NO_PAD
        .decode(&stored.nonce)
        .ok()
        .filter(|n| n.len() == 12)
        .ok_or_else(|| parse("bad nonce"))?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(&stored.ciphertext)
        .map_err(|_| parse("bad ciphertext"))?;
    let plaintext = Zeroizing::new(
        ChaCha20Poly1305::new(Key::from_slice(key))
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: user_id.as_bytes(),
                },
            )
            .map_err(|_| parse("it does not decrypt"))?,
    );
    let encoded = std::str::from_utf8(&plaintext).map_err(|_| parse("not text"))?;
    TOTPSecret::from_base32(encoded)
        .map(|s| s.with_algorithm(stored.algorithm))
        .ok_or_else(|| parse("not a secret"))
}

fn challenge_key(challenge_id: &str) -> String {
    // `challenge_id` is not a password. It is a 256-bit random token from
    // `create_challenge`, which fills 32 bytes from the CSPRNG. This hash is
    // only the cache lookup key, so a cache dump does not reveal live ids.
    // Slow password hashes (Argon2 and the like) are for low-entropy, guessable
    // secrets. For a 256-bit random value, SHA-256 is correct and preimage-resistant.
    // Re-check this if the id ever becomes user-chosen or shorter.
    let digest = Sha256::digest(challenge_id.as_bytes());
    let mut key = String::from("mfa:challenge:");
    for byte in digest {
        // Writing to a `String` cannot fail.
        let _ = write!(key, "{byte:02x}");
    }
    key
}

/// The record stored for one MFA challenge, under the SHA-256 of its id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaChallengeRecord {
    /// The user whose password was proven.
    pub user_id: String,
    /// What is needed to finish the login.
    pub pending_login: PendingLogin,
    /// Unix time in milliseconds after which the challenge is not valid.
    pub expires_at: i64,
}

/// MFA service — TOTP secrets and challenges, kept in the shared cache.
pub struct MfaService<KV: KeyValueStore> {
    storage: Arc<HandlerStorage<KV>>,
    keys: TotpKeyRing,
    algorithm: TotpAlgorithm,
    ctx: Ctx,
}

impl<KV: KeyValueStore> MfaService<KV> {
    /// `keys` encrypt the TOTP secrets (`IdpConfig::totp_keys`). With no
    /// active key, enrolling a secret fails.
    #[must_use]
    pub fn new(storage: Arc<HandlerStorage<KV>>, keys: TotpKeyRing, ctx: Ctx) -> Self {
        Self {
            storage,
            keys,
            algorithm: TotpAlgorithm::default(),
            ctx,
        }
    }

    /// The hash new TOTP secrets use (default SHA-1).
    #[must_use]
    pub fn with_algorithm(mut self, algorithm: TotpAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Store the TOTP secret of a user, encrypted. Replaces an earlier secret
    /// and resets the replay guard. The caller must have proved the user
    /// (enrollment endpoints do; so does an operator tool).
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if no encryption key is configured or the
    /// cache write fails.
    pub async fn enroll_totp(
        &self,
        user_id: &str,
        secret: &TOTPSecret,
    ) -> Result<(), StorageOpError> {
        self.store_enrolled(user_id, secret, 0).await
    }

    async fn store_enrolled(
        &self,
        user_id: &str,
        secret: &TOTPSecret,
        last_step: u64,
    ) -> Result<(), StorageOpError> {
        let encrypted = encrypt_secret(&self.keys, user_id, secret)?;
        let cache_err =
            |e: foundation_db::StorageError| StorageOpError::Query(format!("cache set: {e}"));
        self.storage
            .cache
            .set(&step_key(user_id), last_step)
            .await
            .map_err(cache_err)?;
        self.storage
            .cache
            .set(&totp_key(user_id), encrypted)
            .await
            .map_err(cache_err)
    }

    /// True if the user has an enrolled TOTP secret.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the cache call fails.
    pub async fn has_totp(&self, user_id: &str) -> Result<bool, StorageOpError> {
        self.storage
            .cache
            .exists(&totp_key(user_id))
            .await
            .map_err(|e| StorageOpError::Query(format!("cache exists: {e}")))
    }

    /// The stored TOTP secret of a user, decrypted, if the user has enrolled one.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the cache call fails or the stored value
    /// does not decrypt (wrong or missing key).
    pub async fn totp_secret(&self, user_id: &str) -> Result<Option<TOTPSecret>, StorageOpError> {
        let Some(stored) = self
            .storage
            .cache
            .get::<EncryptedSecret>(&totp_key(user_id))
            .await
            .map_err(|e| StorageOpError::Query(format!("cache get: {e}")))?
        else {
            return Ok(None);
        };
        decrypt_secret(&self.keys, user_id, &stored).map(Some)
    }

    /// Check a one-time code for an enrolled user, and spend it.
    ///
    /// The code is valid only for a time step later than the last accepted one,
    /// so a code works once. The last step is taken from the cache with one
    /// atomic `take` and written back, so two simultaneous requests cannot both
    /// pass; the second one fails closed. Returns `false` for a wrong code, a
    /// replayed code, or a user with no secret.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if a cache call fails or the secret does not
    /// decrypt.
    pub async fn verify_totp_code(
        &self,
        user_id: &str,
        code: &str,
    ) -> Result<bool, StorageOpError> {
        let Some(secret) = self.totp_secret(user_id).await? else {
            return Ok(false);
        };
        let cache_err =
            |e: foundation_db::StorageError| StorageOpError::Query(format!("cache: {e}"));
        let Some(last) = self
            .storage
            .cache
            .take::<u64>(&step_key(user_id))
            .await
            .map_err(cache_err)?
        else {
            // Another request holds the step right now (or it is lost).
            return Ok(false);
        };
        let accepted = secret
            .verify_step(code.trim(), TOTP_TOLERANCE_STEPS, &self.ctx)
            .filter(|step| *step > last);
        self.storage
            .cache
            .set(&step_key(user_id), accepted.unwrap_or(last))
            .await
            .map_err(cache_err)?;
        Ok(accepted.is_some())
    }

    /// Start enrolling a new TOTP secret for a user: make a secret, keep it
    /// encrypted until the user confirms it with a first code, and return it so
    /// the user can add it to an authenticator app. Replaces an earlier pending
    /// secret. The caller must have proved the user, and, if the user is
    /// already enrolled, the current code.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if no encryption key is configured or a cache
    /// call fails.
    pub async fn start_enrollment(
        &self,
        user_id: &str,
        ttl: Duration,
    ) -> Result<TotpEnrollment, StorageOpError> {
        let secret = TOTPSecret::generate().with_algorithm(self.algorithm);
        let pending = PendingEnrollment {
            secret: encrypt_secret(&self.keys, user_id, &secret)?,
            expires_at: now_millis(&self.ctx)
                .saturating_add(i64::try_from(ttl.as_millis()).unwrap_or(i64::MAX)),
        };
        self.storage
            .cache
            .set(&pending_key(user_id), pending)
            .await
            .map_err(|e| StorageOpError::Query(format!("cache set: {e}")))?;
        Ok(TotpEnrollment {
            secret_base32: secret.to_base32(),
            algorithm: self.algorithm,
        })
    }

    /// Finish enrolling: check the first code against the pending secret and,
    /// if it is right, store the secret as the user's TOTP secret. The pending
    /// secret is taken (atomically) whatever the result, so one start allows
    /// one attempt. Returns `false` for a wrong code, an expired or missing
    /// pending secret.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if a cache call fails or the pending secret
    /// does not decrypt.
    pub async fn confirm_enrollment(
        &self,
        user_id: &str,
        code: &str,
    ) -> Result<bool, StorageOpError> {
        let Some(pending) = self
            .storage
            .cache
            .take::<PendingEnrollment>(&pending_key(user_id))
            .await
            .map_err(|e| StorageOpError::Query(format!("cache take: {e}")))?
        else {
            return Ok(false);
        };
        if now_millis(&self.ctx) >= pending.expires_at {
            return Ok(false);
        }
        let secret = decrypt_secret(&self.keys, user_id, &pending.secret)?;
        let Some(step) = secret.verify_step(code.trim(), TOTP_TOLERANCE_STEPS, &self.ctx) else {
            return Ok(false);
        };
        // The first code is spent: it cannot be used to log in.
        self.store_enrolled(user_id, &secret, step).await?;
        Ok(true)
    }

    /// Issue a challenge for a user whose password was proven. Returns the
    /// challenge id (shown once; only its SHA-256 is stored).
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the cache write fails.
    pub async fn create_challenge(
        &self,
        user_id: &str,
        pending_login: PendingLogin,
        ttl: Duration,
    ) -> Result<String, StorageOpError> {
        let id = {
            let mut bytes = [0u8; 32];
            foundation_compact::rng::rng().fill_bytes(&mut bytes);
            URL_SAFE_NO_PAD.encode(bytes)
        };
        let record = MfaChallengeRecord {
            user_id: user_id.to_string(),
            pending_login,
            expires_at: now_millis(&self.ctx)
                .saturating_add(i64::try_from(ttl.as_millis()).unwrap_or(i64::MAX)),
        };
        self.storage
            .cache
            .set(&challenge_key(&id), record)
            .await
            .map_err(|e| StorageOpError::Query(format!("cache set: {e}")))?;
        Ok(id)
    }

    /// Use a challenge: the record is removed by one atomic
    /// `KeyValueStore::take` before it is returned, so one challenge allows one
    /// code attempt even when many requests send it at the same time. An
    /// unknown, used or expired challenge gives `None`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the cache call fails.
    pub async fn take_challenge(
        &self,
        challenge_id: &str,
    ) -> Result<Option<MfaChallengeRecord>, StorageOpError> {
        let Some(record) = self
            .storage
            .cache
            .take::<MfaChallengeRecord>(&challenge_key(challenge_id))
            .await
            .map_err(|e| StorageOpError::Query(format!("cache take: {e}")))?
        else {
            return Ok(None);
        };
        if now_millis(&self.ctx) >= record.expires_at {
            return Ok(None);
        }
        Ok(Some(record))
    }
}
