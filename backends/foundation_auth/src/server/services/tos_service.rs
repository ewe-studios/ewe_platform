//! Terms of Service service — versioning, display, and acceptance tracking.

use std::sync::Arc;
use std::sync::Mutex;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::super::models::{TosAcceptance, TosVersion};
use super::super::storage::{HandlerStorage, StorageOpError};
use foundation_db::KeyValueStore;

/// ToS service — manages ToS versions and user acceptances.
pub struct TosService<KV: KeyValueStore> {
    storage: Arc<HandlerStorage<KV>>,
    /// In-memory ToS versions (in production, loaded from DB).
    versions: Mutex<Vec<TosVersion>>,
}

impl<KV: KeyValueStore> TosService<KV> {
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
        let versions = self.versions.lock().unwrap();
        versions.iter().max_by_key(|v| v.effective_from).cloned()
    }

    /// Check if a user has accepted the latest ToS.
    pub fn needs_acceptance(&self, user_id: &str) -> bool {
        let versions = self.versions.lock().unwrap();
        let latest = match versions.iter().max_by_key(|v| v.effective_from) {
            Some(v) => v,
            None => return false,
        };
        match super::super::storage::find_tos_acceptance(
            self.storage.query_store.as_ref(),
            user_id,
            &latest.version,
        ) {
            Ok(Some(_)) => false,
            Ok(None) | Err(_) => true,
        }
    }

    /// Record a user's acceptance of the current ToS.
    pub fn accept(&self, user_id: &str, ip_address: Option<&str>) -> Result<(), StorageOpError> {
        let versions = self.versions.lock().unwrap();
        let latest = match versions.iter().max_by_key(|v| v.effective_from) {
            Some(v) => v,
            None => return Ok(()),
        };

        let acceptance = TosAcceptance {
            user_id: user_id.to_string(),
            tos_version: latest.version.clone(),
            accepted_at: Utc::now().timestamp_millis(),
            ip_address: ip_address.map(|s| s.to_string()),
        };

        super::super::storage::store_tos_acceptance(
            self.storage.query_store.as_ref(),
            &acceptance,
        )
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
