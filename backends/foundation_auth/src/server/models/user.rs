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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_user() -> User {
        User {
            id: "user_1".into(),
            email: "test@example.com".into(),
            username: None,
            password_hash: Some("$argon2id$hash".into()),
            email_verified: false,
            email_verified_at: None,
            created_at: Utc::now().timestamp_millis(),
            updated_at: Utc::now().timestamp_millis(),
            metadata: None,
            failed_login_attempts: 0,
            locked_until: None,
            deleted_at: None,
        }
    }

    #[test]
    fn test_not_locked() {
        let user = test_user();
        assert!(!user.is_locked());
    }

    #[test]
    fn test_locked_future() {
        let mut user = test_user();
        user.locked_until = Some(Utc::now().timestamp_millis() + 60_000);
        assert!(user.is_locked());
    }

    #[test]
    fn test_locked_past() {
        let mut user = test_user();
        user.locked_until = Some(Utc::now().timestamp_millis() - 60_000);
        assert!(!user.is_locked());
    }

    #[test]
    fn test_record_failed_attempts_locks() {
        let mut user = test_user();
        for _ in 0..5 {
            user.record_failed_attempt(5, Duration::from_secs(900));
        }
        assert!(user.is_locked());
        assert_eq!(user.failed_login_attempts, 5);
    }

    #[test]
    fn test_reset_failed_attempts() {
        let mut user = test_user();
        user.failed_login_attempts = 3;
        user.locked_until = Some(Utc::now().timestamp_millis() + 60_000);
        user.reset_failed_attempts();
        assert_eq!(user.failed_login_attempts, 0);
        assert!(!user.is_locked());
    }
}
