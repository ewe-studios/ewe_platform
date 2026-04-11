//! Cleanup and maintenance operations for storage backends.
//!
//! Provides methods to clean up expired data such as:
//! - Expired sessions (past absolute expiration or inactive timeout)
//! - Expired JWT tokens (past refresh token expiration)
//! - Expired rate limits (outside the sliding window)
//! - Expired verification tokens, OAuth states, magic links, and email OTPs

use crate::errors::{StorageError, StorageResult};
use crate::storage_provider::QueryStore;

/// Cleanup operations for storage maintenance.
pub trait StorageCleanup: QueryStore {
    /// Delete expired sessions.
    ///
    /// Sessions are considered expired if:
    /// - Past their `expires_at` timestamp
    /// - Inactive for longer than `max_inactive_seconds` (if provided)
    ///
    /// Returns the number of deleted sessions.
    fn cleanup_expired_sessions(
        &self,
        now: i64,
        max_inactive_seconds: Option<i64>,
    ) -> StorageResult<u64> {
        let sql = match max_inactive_seconds {
            Some(_inactive_timeout) => format!(
                "DELETE FROM sessions WHERE expires_at < ? OR (last_active_at IS NOT NULL AND last_active_at < ?)",
            ),
            None => "DELETE FROM sessions WHERE expires_at < ?".to_string(),
        };

        let params = match max_inactive_seconds {
            Some(inactive_timeout) => vec![
                crate::storage_provider::DataValue::Integer(now),
                crate::storage_provider::DataValue::Integer(now - inactive_timeout),
            ],
            None => vec![crate::storage_provider::DataValue::Integer(now)],
        };

        let stream = self.execute(&sql, &params)?;
        for item in stream {
            if let foundation_core::valtron::Stream::Next(result) = item {
                return result.map_err(|e| StorageError::Generic(format!("Cleanup failed: {e}")));
            }
        }
        Err(StorageError::Generic("No result from cleanup".to_string()))
    }

    /// Delete expired JWT tokens (refresh tokens past expiration).
    ///
    /// Returns the number of deleted tokens.
    fn cleanup_expired_jwt_tokens(&self, now: i64) -> StorageResult<u64> {
        let sql = "DELETE FROM jwt_tokens WHERE expires_at < ?";
        let params = vec![crate::storage_provider::DataValue::Integer(now)];

        let stream = self.execute(sql, &params)?;
        for item in stream {
            if let foundation_core::valtron::Stream::Next(result) = item {
                return result.map_err(|e| StorageError::Generic(format!("Cleanup failed: {e}")));
            }
        }
        Err(StorageError::Generic("No result from cleanup".to_string()))
    }

    /// Delete expired verification tokens.
    ///
    /// Returns the number of deleted tokens.
    fn cleanup_expired_verification_tokens(&self, now: i64) -> StorageResult<u64> {
        let sql = "DELETE FROM verification_tokens WHERE expires_at < ?";
        let params = vec![crate::storage_provider::DataValue::Integer(now)];

        let stream = self.execute(sql, &params)?;
        for item in stream {
            if let foundation_core::valtron::Stream::Next(result) = item {
                return result.map_err(|e| StorageError::Generic(format!("Cleanup failed: {e}")));
            }
        }
        Err(StorageError::Generic("No result from cleanup".to_string()))
    }

    /// Delete expired OAuth states (PKCE states past expiration).
    ///
    /// Returns the number of deleted states.
    fn cleanup_expired_oauth_states(&self, now: i64) -> StorageResult<u64> {
        let sql = "DELETE FROM oauth_states WHERE expires_at < ?";
        let params = vec![crate::storage_provider::DataValue::Integer(now)];

        let stream = self.execute(sql, &params)?;
        for item in stream {
            if let foundation_core::valtron::Stream::Next(result) = item {
                return result.map_err(|e| StorageError::Generic(format!("Cleanup failed: {e}")));
            }
        }
        Err(StorageError::Generic("No result from cleanup".to_string()))
    }

    /// Delete expired magic links.
    ///
    /// Returns the number of deleted links.
    fn cleanup_expired_magic_links(&self, now: i64) -> StorageResult<u64> {
        let sql = "DELETE FROM magic_links WHERE expires_at < ?";
        let params = vec![crate::storage_provider::DataValue::Integer(now)];

        let stream = self.execute(sql, &params)?;
        for item in stream {
            if let foundation_core::valtron::Stream::Next(result) = item {
                return result.map_err(|e| StorageError::Generic(format!("Cleanup failed: {e}")));
            }
        }
        Err(StorageError::Generic("No result from cleanup".to_string()))
    }

    /// Delete expired email OTPs.
    ///
    /// Returns the number of deleted OTPs.
    fn cleanup_expired_email_otps(&self, now: i64) -> StorageResult<u64> {
        let sql = "DELETE FROM email_otps WHERE expires_at < ?";
        let params = vec![crate::storage_provider::DataValue::Integer(now)];

        let stream = self.execute(sql, &params)?;
        for item in stream {
            if let foundation_core::valtron::Stream::Next(result) = item {
                return result.map_err(|e| StorageError::Generic(format!("Cleanup failed: {e}")));
            }
        }
        Err(StorageError::Generic("No result from cleanup".to_string()))
    }

    /// Clean up old rate limit entries.
    ///
    /// Rate limits outside the window are deleted.
    /// Returns the number of deleted entries.
    fn cleanup_old_rate_limits(&self, window_seconds: i64) -> StorageResult<u64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cutoff = now - window_seconds;

        let sql = "DELETE FROM rate_limits WHERE window_start < ?";
        let params = vec![crate::storage_provider::DataValue::Integer(cutoff)];

        let stream = self.execute(sql, &params)?;
        for item in stream {
            if let foundation_core::valtron::Stream::Next(result) = item {
                return result.map_err(|e| StorageError::Generic(format!("Cleanup failed: {e}")));
            }
        }
        Err(StorageError::Generic("No result from cleanup".to_string()))
    }

    /// Run all cleanup operations in one call.
    ///
    /// Returns a `CleanupStats` with counts of deleted items.
    fn run_full_cleanup(&self) -> StorageResult<CleanupStats> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut stats = CleanupStats::default();

        // Sessions: expire after 7 days, inactive for 24h
        stats.sessions_deleted = self.cleanup_expired_sessions(now, Some(86400))?;

        // JWT tokens: expire after refresh token expiration (30 days)
        stats.jwt_tokens_deleted = self.cleanup_expired_jwt_tokens(now - 2592000)?;

        // Verification tokens: 1 hour expiration
        stats.verification_tokens_deleted = self.cleanup_expired_verification_tokens(now - 3600)?;

        // OAuth states: 10 minute expiration
        stats.oauth_states_deleted = self.cleanup_expired_oauth_states(now - 600)?;

        // Magic links: 1 hour expiration
        stats.magic_links_deleted = self.cleanup_expired_magic_links(now - 3600)?;

        // Email OTPs: 10 minute expiration
        stats.email_otps_deleted = self.cleanup_expired_email_otps(now - 600)?;

        // Rate limits: clean up entries older than 1 hour
        stats.rate_limits_deleted = self.cleanup_old_rate_limits(3600)?;

        Ok(stats)
    }
}

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

// Implement for all QueryStore backends
impl<T: QueryStore> StorageCleanup for T {}
