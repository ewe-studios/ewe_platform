//! Terms of Service service — versioning, display, and acceptance tracking.

use std::sync::Arc;
use std::sync::Mutex;

use chrono::Utc;

use super::super::models::{TosAcceptance, TosVersion};
use super::super::storage::{self, HandlerStorage, StorageOpError};
use foundation_db::AsyncKeyValueStore;

/// ToS service — manages ToS versions and user acceptances.
pub struct TosService<KV: AsyncKeyValueStore> {
    storage: Arc<HandlerStorage<KV>>,
    /// In-memory ToS versions (in production, loaded from DB).
    versions: Mutex<Vec<TosVersion>>,
}

impl<KV: AsyncKeyValueStore> TosService<KV> {
    #[must_use]
    pub fn new(storage: Arc<HandlerStorage<KV>>) -> Self {
        Self {
            storage,
            versions: Mutex::new(Vec::new()),
        }
    }

    /// Register a new ToS version.
    pub fn add_version(&self, version: TosVersion) {
        let mut versions = self.versions.lock().unwrap();
        versions.push(version);
    }

    /// Get the latest ToS version.
    pub fn get_latest(&self) -> Option<TosVersion> {
        self.latest()
    }

    /// The latest ToS version, if any is registered.
    fn latest(&self) -> Option<TosVersion> {
        let versions = self.versions.lock().unwrap();
        versions.iter().max_by_key(|v| v.effective_from).cloned()
    }

    /// Check if a user has accepted the latest ToS.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the acceptance lookup fails.
    pub async fn needs_acceptance(&self, user_id: &str) -> Result<bool, StorageOpError> {
        let Some(latest) = self.latest() else {
            return Ok(false);
        };
        let accepted =
            storage::find_tos_acceptance(self.storage.query_store.as_ref(), user_id, &latest.version)
                .await?;
        Ok(accepted.is_none())
    }

    /// Record a user's acceptance of the current ToS.
    ///
    /// # Errors
    ///
    /// Returns [`StorageOpError`] if the acceptance can't be stored.
    pub async fn accept(&self, user_id: &str, ip_address: Option<&str>) -> Result<(), StorageOpError> {
        let Some(latest) = self.latest() else {
            return Ok(());
        };

        let acceptance = TosAcceptance {
            user_id: user_id.to_string(),
            tos_version: latest.version,
            accepted_at: Utc::now().timestamp_millis(),
            ip_address: ip_address.map(|s| s.to_string()),
        };

        storage::store_tos_acceptance(self.storage.query_store.as_ref(), &acceptance).await
    }

    /// Generate a ToS acceptance code for the login flow.
    pub fn generate_tos_code(&self, user_id: &str) -> String {
        let code = uuid::Uuid::new_v4().to_string();
        let _user_id = user_id;
        code
    }

    /// Verify a ToS acceptance code.
    pub fn verify_tos_code(&self, _code: &str, user_id: &str) -> bool {
        // In production, look up the code and verify it matches the user.
        let _ = user_id;
        true
    }
}
