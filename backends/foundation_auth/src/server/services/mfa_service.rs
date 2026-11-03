//! Multi-factor login service — TOTP secrets and one-time MFA challenges.
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

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use foundation_compact::rng::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::super::models::PendingLogin;
use super::super::storage::{HandlerStorage, StorageOpError};
use crate::shared::two_factor::TOTPSecret;
use foundation_db::KeyValueStore;

/// How many 30-second steps before and after now a TOTP code may be off.
const TOTP_TOLERANCE_STEPS: u64 = 1;

fn totp_key(user_id: &str) -> String {
    format!("mfa:totp:{user_id}")
}

fn challenge_key(challenge_id: &str) -> String {
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
}

impl<KV: KeyValueStore> MfaService<KV> {
    #[must_use]
    pub fn new(storage: Arc<HandlerStorage<KV>>) -> Self {
        Self { storage }
    }

    /// Store the TOTP secret of a user (base32). Replaces an earlier secret.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the cache write fails.
    pub async fn enroll_totp(
        &self,
        user_id: &str,
        secret: &TOTPSecret,
    ) -> Result<(), StorageOpError> {
        self.storage
            .cache
            .set(&totp_key(user_id), secret.to_base32())
            .await
            .map_err(|e| StorageOpError::Query(format!("cache set: {e}")))
    }

    /// The stored TOTP secret of a user, if the user has enrolled one.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the cache call fails or the stored value
    /// is not a valid secret.
    pub async fn totp_secret(&self, user_id: &str) -> Result<Option<TOTPSecret>, StorageOpError> {
        let Some(encoded) = self
            .storage
            .cache
            .get::<String>(&totp_key(user_id))
            .await
            .map_err(|e| StorageOpError::Query(format!("cache get: {e}")))?
        else {
            return Ok(None);
        };
        TOTPSecret::from_base32(&encoded)
            .map(Some)
            .ok_or_else(|| StorageOpError::Parse("stored TOTP secret is not valid".into()))
    }

    /// Check a one-time code against a secret.
    #[must_use]
    pub fn verify_code(secret: &TOTPSecret, code: &str) -> bool {
        secret.verify(code.trim(), TOTP_TOLERANCE_STEPS)
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
            expires_at: Utc::now()
                .timestamp_millis()
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
        if Utc::now().timestamp_millis() >= record.expires_at {
            return Ok(None);
        }
        Ok(Some(record))
    }
}
