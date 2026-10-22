//! Cleanup and maintenance operations for storage backends.
//!
//! Deletes expired rows from the auth tables:
//! - sessions past their expiry or idle for too long;
//! - JWT tokens, verification tokens, OAuth states, magic links and email
//!   OTPs past their expiry;
//! - rate-limit windows that have closed.
//!
//! Every operation is async over [`QueryStore`]; [`StorageCleanup`]
//! is implemented for every async SQL store. Timestamps are compared in Unix
//! **seconds** (the unit the callers write into `expires_at`), and every
//! retention period comes from [`CleanupConfig`].

use std::time::Duration;

use crate::core::backends::unix_now_secs;
use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{QueryStore, DataValue};

/// How long expired rows are kept before a full cleanup deletes them.
///
/// The defaults are the values `run_full_cleanup` used before they became
/// configuration; each is documented on its field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupConfig {
    /// A session not active for this long is deleted even if it hasn't
    /// expired. Default: 24 hours.
    pub session_max_inactive: Duration,
    /// How long an expired JWT (refresh) token is kept. Default: 30 days, the
    /// refresh-token lifetime.
    pub jwt_token_retention: Duration,
    /// How long an expired verification token is kept. Default: 1 hour.
    pub verification_token_retention: Duration,
    /// How long an expired OAuth (PKCE) state is kept. Default: 10 minutes.
    pub oauth_state_retention: Duration,
    /// How long an expired magic link is kept. Default: 1 hour.
    pub magic_link_retention: Duration,
    /// How long an expired email OTP is kept. Default: 10 minutes.
    pub email_otp_retention: Duration,
    /// Rate-limit windows that started longer ago than this are deleted.
    /// Default: 1 hour.
    pub rate_limit_window: Duration,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            session_max_inactive: Duration::from_secs(24 * 60 * 60),
            jwt_token_retention: Duration::from_secs(30 * 24 * 60 * 60),
            verification_token_retention: Duration::from_secs(60 * 60),
            oauth_state_retention: Duration::from_secs(10 * 60),
            magic_link_retention: Duration::from_secs(60 * 60),
            email_otp_retention: Duration::from_secs(10 * 60),
            rate_limit_window: Duration::from_secs(60 * 60),
        }
    }
}

impl CleanupConfig {
    /// Set [`session_max_inactive`](Self::session_max_inactive).
    #[must_use]
    pub fn with_session_max_inactive(mut self, d: Duration) -> Self {
        self.session_max_inactive = d;
        self
    }

    /// Set [`jwt_token_retention`](Self::jwt_token_retention).
    #[must_use]
    pub fn with_jwt_token_retention(mut self, d: Duration) -> Self {
        self.jwt_token_retention = d;
        self
    }

    /// Set [`verification_token_retention`](Self::verification_token_retention).
    #[must_use]
    pub fn with_verification_token_retention(mut self, d: Duration) -> Self {
        self.verification_token_retention = d;
        self
    }

    /// Set [`oauth_state_retention`](Self::oauth_state_retention).
    #[must_use]
    pub fn with_oauth_state_retention(mut self, d: Duration) -> Self {
        self.oauth_state_retention = d;
        self
    }

    /// Set [`magic_link_retention`](Self::magic_link_retention).
    #[must_use]
    pub fn with_magic_link_retention(mut self, d: Duration) -> Self {
        self.magic_link_retention = d;
        self
    }

    /// Set [`email_otp_retention`](Self::email_otp_retention).
    #[must_use]
    pub fn with_email_otp_retention(mut self, d: Duration) -> Self {
        self.email_otp_retention = d;
        self
    }

    /// Set [`rate_limit_window`](Self::rate_limit_window).
    #[must_use]
    pub fn with_rate_limit_window(mut self, d: Duration) -> Self {
        self.rate_limit_window = d;
        self
    }
}

/// A duration as whole seconds in the `i64` the SQL comparisons use.
fn secs(d: Duration) -> StorageResult<i64> {
    i64::try_from(d.as_secs())
        .map_err(|_| StorageError::Backend(format!("cleanup period too large: {d:?}")))
}

/// `now - period`, in seconds.
fn cutoff(now: i64, period: Duration) -> StorageResult<i64> {
    Ok(now.saturating_sub(secs(period)?))
}

/// Cleanup operations for storage maintenance, on any async SQL store.
#[async_trait::async_trait]
pub trait StorageCleanup: QueryStore {
    /// Delete expired sessions, and (with `max_inactive`) sessions idle for
    /// longer than that. `now` is Unix seconds. Returns the number deleted.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database query fails.
    async fn cleanup_expired_sessions(&self, now: i64, max_inactive: Option<Duration>) -> StorageResult<u64> {
        match max_inactive {
            Some(inactive) => {
                self.execute_async(
                    "DELETE FROM sessions WHERE expires_at < ? OR (last_active_at IS NOT NULL AND last_active_at < ?)",
                    &[DataValue::Integer(now), DataValue::Integer(cutoff(now, inactive)?)],
                )
                .await
            }
            None => {
                self.execute_async("DELETE FROM sessions WHERE expires_at < ?", &[DataValue::Integer(now)])
                    .await
            }
        }
    }

    /// Delete JWT tokens that expired before `before` (Unix seconds).
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database query fails.
    async fn cleanup_expired_jwt_tokens(&self, before: i64) -> StorageResult<u64> {
        self.execute_async("DELETE FROM jwt_tokens WHERE expires_at < ?", &[DataValue::Integer(before)])
            .await
    }

    /// Delete verification tokens that expired before `before` (Unix seconds).
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database query fails.
    async fn cleanup_expired_verification_tokens(&self, before: i64) -> StorageResult<u64> {
        self.execute_async(
            "DELETE FROM verification_tokens WHERE expires_at < ?",
            &[DataValue::Integer(before)],
        )
        .await
    }

    /// Delete OAuth states that expired before `before` (Unix seconds).
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database query fails.
    async fn cleanup_expired_oauth_states(&self, before: i64) -> StorageResult<u64> {
        self.execute_async("DELETE FROM oauth_states WHERE expires_at < ?", &[DataValue::Integer(before)])
            .await
    }

    /// Delete magic links that expired before `before` (Unix seconds).
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database query fails.
    async fn cleanup_expired_magic_links(&self, before: i64) -> StorageResult<u64> {
        self.execute_async("DELETE FROM magic_links WHERE expires_at < ?", &[DataValue::Integer(before)])
            .await
    }

    /// Delete email OTPs that expired before `before` (Unix seconds).
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database query fails.
    async fn cleanup_expired_email_otps(&self, before: i64) -> StorageResult<u64> {
        self.execute_async("DELETE FROM email_otps WHERE expires_at < ?", &[DataValue::Integer(before)])
            .await
    }

    /// Delete rate-limit windows that started more than `window` before
    /// `now` (Unix seconds).
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database query fails.
    async fn cleanup_old_rate_limits(&self, now: i64, window: Duration) -> StorageResult<u64> {
        self.execute_async(
            "DELETE FROM rate_limits WHERE window_start < ?",
            &[DataValue::Integer(cutoff(now, window)?)],
        )
        .await
    }

    /// Run every cleanup with the retention periods in `config`.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the clock can't be read or any cleanup
    /// query fails.
    async fn run_full_cleanup(&self, config: &CleanupConfig) -> StorageResult<CleanupStats> {
        let now = i64::try_from(unix_now_secs()?)
            .map_err(|_| StorageError::Backend("clock out of range".into()))?;
        Ok(CleanupStats {
            sessions_deleted: self
                .cleanup_expired_sessions(now, Some(config.session_max_inactive))
                .await?,
            jwt_tokens_deleted: self
                .cleanup_expired_jwt_tokens(cutoff(now, config.jwt_token_retention)?)
                .await?,
            verification_tokens_deleted: self
                .cleanup_expired_verification_tokens(cutoff(now, config.verification_token_retention)?)
                .await?,
            oauth_states_deleted: self
                .cleanup_expired_oauth_states(cutoff(now, config.oauth_state_retention)?)
                .await?,
            magic_links_deleted: self
                .cleanup_expired_magic_links(cutoff(now, config.magic_link_retention)?)
                .await?,
            email_otps_deleted: self
                .cleanup_expired_email_otps(cutoff(now, config.email_otp_retention)?)
                .await?,
            rate_limits_deleted: self.cleanup_old_rate_limits(now, config.rate_limit_window).await?,
        })
    }
}

impl<T: QueryStore + ?Sized> StorageCleanup for T {}

/// Statistics from a cleanup run.
#[derive(Debug, Default, Clone)]
pub struct CleanupStats {
    pub sessions_deleted: u64,
    pub jwt_tokens_deleted: u64,
    pub verification_tokens_deleted: u64,
    pub oauth_states_deleted: u64,
    pub magic_links_deleted: u64,
    pub email_otps_deleted: u64,
    pub rate_limits_deleted: u64,
}

impl CleanupStats {
    /// Total number of items deleted.
    #[must_use]
    pub fn total_deleted(&self) -> u64 {
        self.sessions_deleted
            + self.jwt_tokens_deleted
            + self.verification_tokens_deleted
            + self.oauth_states_deleted
            + self.magic_links_deleted
            + self.email_otps_deleted
            + self.rate_limits_deleted
    }

    /// Human-readable summary.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "Cleanup complete: {} sessions, {} JWT tokens, {} verification tokens, {} OAuth states, {} magic links, {} email OTPs, {} rate limits",
            self.sessions_deleted,
            self.jwt_tokens_deleted,
            self.verification_tokens_deleted,
            self.oauth_states_deleted,
            self.magic_links_deleted,
            self.email_otps_deleted,
            self.rate_limits_deleted
        )
    }
}
