//! User entity for the IdP server.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub username: Option<String>,
    pub password_hash: Option<String>,
    pub email_verified: bool,
    pub email_verified_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub metadata: Option<serde_json::Value>,
    pub failed_login_attempts: u32,
    pub locked_until: Option<i64>,
    pub deleted_at: Option<i64>,
}

impl User {
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.locked_until
            .is_some_and(|until| Utc::now().timestamp_millis() < until)
    }

    pub fn record_failed_attempt(&mut self, max_attempts: u32, lockout_duration: Duration) {
        self.failed_login_attempts += 1;
        if self.failed_login_attempts >= max_attempts {
            self.locked_until =
                Some(Utc::now().timestamp_millis() + lockout_duration.as_millis() as i64);
        }
    }

    pub fn reset_failed_attempts(&mut self) {
        self.failed_login_attempts = 0;
        self.locked_until = None;
    }

    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    #[must_use]
    pub fn has_password(&self) -> bool {
        self.password_hash.is_some()
    }
}

