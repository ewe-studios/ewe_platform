//! Terms of Service service — versions, acceptances and `ToS` codes.
//!
//! This service owns three things:
//!
//! - the `ToS` versions (kept in the shared cache, so every handler instance
//!   sees the same versions),
//! - the acceptances (kept in the SQL store), and
//! - the `ToS` codes (kept in the shared cache, as the SHA-256 of the code).
//!
//! A `ToS` code is issued when a login is proven but the user has not accepted
//! the active `ToS` version. The code is 256 random bits. Only its SHA-256 is
//! stored. It is bound to the user, the `ToS` version and the pending login. It
//! expires after `IdpConfig::tos_code_ttl` and it works once.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use crate::shared::clock::now_millis;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use foundation_compact::rng::Rng;
use foundation_context::Ctx;
use sha2::{Digest, Sha256};

use super::super::models::{PendingLogin, TosAcceptance, TosCodeRecord, TosVersion};
use super::super::storage::{self, HandlerStorage, StorageOpError};
use foundation_db::KeyValueStore;

/// Cache key of the list of `ToS` versions.
const VERSIONS_KEY: &str = "tos:versions";

/// Cache key of a `ToS` code: derived from the SHA-256 of the code, so the
/// code itself is never stored.
fn code_key(code: &str) -> String {
    let digest = Sha256::digest(code.as_bytes());
    let mut key = String::with_capacity("tos:code:".len() + digest.len() * 2);
    key.push_str("tos:code:");
    for byte in digest {
        // Writing to a `String` cannot fail.
        let _ = write!(key, "{byte:02x}");
    }
    key
}

/// Why a `ToS` acceptance was refused.
#[derive(Debug)]
pub enum TosAcceptError {
    /// The version the caller gave is not the active version.
    VersionMismatch {
        /// The active version.
        active: String,
    },
    /// No `ToS` version is configured, so there is nothing to accept.
    NoActiveVersion,
    /// A storage call failed.
    Storage(StorageOpError),
}

impl core::fmt::Display for TosAcceptError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::VersionMismatch { active } => {
                write!(f, "ToS version is not the active version ({active})")
            }
            Self::NoActiveVersion => write!(f, "No ToS version is configured"),
            Self::Storage(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TosAcceptError {}

impl From<StorageOpError> for TosAcceptError {
    fn from(e: StorageOpError) -> Self {
        Self::Storage(e)
    }
}

/// A `ToS` code issued to a client, with the version it must accept.
#[derive(Debug, Clone)]
pub struct TosChallenge {
    /// The code to give back to `POST /auth/v1/tos/accept`. Shown once.
    pub code: String,
    /// The `ToS` version the user must accept.
    pub tos_version: String,
}

/// `ToS` service — manages `ToS` versions, user acceptances and `ToS` codes.
pub struct TosService<KV: KeyValueStore> {
    storage: Arc<HandlerStorage<KV>>,
    ctx: Ctx,
}

impl<KV: KeyValueStore> TosService<KV> {
    #[must_use]
    pub fn new(storage: Arc<HandlerStorage<KV>>, ctx: Ctx) -> Self {
        Self { storage, ctx }
    }

    async fn versions(&self) -> Result<Vec<TosVersion>, StorageOpError> {
        let stored = self
            .storage
            .cache
            .get::<Vec<TosVersion>>(VERSIONS_KEY)
            .await
            .map_err(|e| StorageOpError::Query(format!("cache get: {e}")))?;
        Ok(stored.unwrap_or_default())
    }

    /// Register a new `ToS` version.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the cache call fails.
    pub async fn add_version(&self, version: TosVersion) -> Result<(), StorageOpError> {
        let mut versions = self.versions().await?;
        versions.push(version);
        self.storage
            .cache
            .set(VERSIONS_KEY, versions)
            .await
            .map_err(|e| StorageOpError::Query(format!("cache set: {e}")))
    }

    /// Get the latest (active) `ToS` version, if any is registered.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the cache call fails.
    pub async fn get_latest(&self) -> Result<Option<TosVersion>, StorageOpError> {
        Ok(self
            .versions()
            .await?
            .into_iter()
            .max_by_key(|v| v.effective_from))
    }

    /// Check if a user has accepted the latest `ToS`. With no `ToS` version
    /// configured, nobody needs to accept anything.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if a lookup fails.
    pub async fn needs_acceptance(&self, user_id: &str) -> Result<bool, StorageOpError> {
        let Some(latest) = self.get_latest().await? else {
            return Ok(false);
        };
        let accepted = storage::find_tos_acceptance(
            self.storage.query_store.as_ref(),
            user_id,
            &latest.version,
        )
        .await?;
        Ok(accepted.is_none())
    }

    /// Record a user's acceptance of the given `ToS` version.
    ///
    /// The version must be the active one. The caller supplies the user id from
    /// an authenticated session or from a `ToS` code, never from a request body.
    ///
    /// # Errors
    ///
    /// Returns [`TosAcceptError`] if no version is configured, if `version` is
    /// not the active version, or if the acceptance cannot be stored.
    pub async fn accept_version(
        &self,
        user_id: &str,
        version: &str,
        ip_address: Option<&str>,
    ) -> Result<(), TosAcceptError> {
        let latest = self
            .get_latest()
            .await?
            .ok_or(TosAcceptError::NoActiveVersion)?;
        if latest.version != version {
            return Err(TosAcceptError::VersionMismatch {
                active: latest.version,
            });
        }
        let acceptance = TosAcceptance {
            user_id: user_id.to_string(),
            tos_version: latest.version,
            accepted_at: now_millis(&self.ctx),
            ip_address: ip_address.map(std::string::ToString::to_string),
        };
        storage::store_tos_acceptance(self.storage.query_store.as_ref(), &acceptance)
            .await
            .map_err(TosAcceptError::Storage)
    }

    /// If the user must accept the active `ToS`, issue a `ToS` code for the
    /// pending login. Returns `None` when there is no gate (no version is
    /// configured, or the user already accepted it).
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if a lookup or the cache write fails.
    pub async fn gate(
        &self,
        user_id: &str,
        pending_login: PendingLogin,
        ttl: Duration,
    ) -> Result<Option<TosChallenge>, StorageOpError> {
        let Some(latest) = self.get_latest().await? else {
            return Ok(None);
        };
        let accepted = storage::find_tos_acceptance(
            self.storage.query_store.as_ref(),
            user_id,
            &latest.version,
        )
        .await?;
        if accepted.is_some() {
            return Ok(None);
        }

        // 256 random bits. The thread-local RNG is not `Send`: it is dropped
        // before the first await.
        let code = {
            let mut bytes = [0u8; 32];
            foundation_compact::rng::rng().fill_bytes(&mut bytes);
            URL_SAFE_NO_PAD.encode(bytes)
        };
        let record = TosCodeRecord {
            user_id: user_id.to_string(),
            tos_version: latest.version.clone(),
            pending_login,
            expires_at: now_millis(&self.ctx)
                .saturating_add(i64::try_from(ttl.as_millis()).unwrap_or(i64::MAX)),
        };
        self.storage
            .cache
            .set(&code_key(&code), record)
            .await
            .map_err(|e| StorageOpError::Query(format!("cache set: {e}")))?;
        Ok(Some(TosChallenge {
            code,
            tos_version: latest.version,
        }))
    }

    /// Use a `ToS` code. The stored record is removed by one atomic
    /// `KeyValueStore::take` before anything else happens, so a code works
    /// once even when many requests send it at the same time. An unknown, used
    /// or expired code gives `None` (the caller cannot tell which).
    ///
    /// The atomic guarantee is the cache backend's (see `KeyValueStore::take`
    /// for the table; Workers KV is not atomic).
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the cache call fails.
    pub async fn take_code(&self, code: &str) -> Result<Option<TosCodeRecord>, StorageOpError> {
        let Some(record) = self
            .storage
            .cache
            .take::<TosCodeRecord>(&code_key(code))
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
