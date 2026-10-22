//! Proxy state persistence (Decision 21).
//!
//! WHY: The proxy must survive restarts. Wraps `foundation_db::FileStateStore`
//! to persist backend drain/pause state, TLS data, and config hashes.
//!
//! WHAT: [`ProxyStateStore`] — load/save proxy state as JSON-serialized
//! `ResourceState` entries, one per domain keyed by "proxy_state".

use std::collections::HashMap;
use std::path::Path;

use foundation_db::core::state::{FileStateStore, StateStore};
use foundation_db::core::state::types::ResourceState;
use foundation_db::core::errors::StorageError;
use serde::{Deserialize, Serialize};

use crate::config::ProxyConfig;

/// Persistent proxy state, serialized as JSON value inside `ResourceState.data`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedProxyState {
    pub backend_states: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert_pem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_pem: Option<String>,
    pub cert_renewed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_hash: Option<String>,
}

pub struct ProxyStateStore {
    store: FileStateStore,
}

const PROXY_STATE_ID: &str = "proxy_state";

impl ProxyStateStore {
    pub fn new(state_dir: &Path, domain: &str) -> Self {
        Self {
            store: FileStateStore::new(state_dir, "foundation_proxy", domain),
        }
    }

    /// Load persisted proxy state, if any.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the state can't be read or decoded.
    pub async fn load(&self) -> Result<Option<PersistedProxyState>, StorageError> {
        match self.store.get(PROXY_STATE_ID).await? {
            Some(resource) => serde_json::from_value(resource.output)
                .map(Some)
                .map_err(|e| StorageError::Serialization(e.to_string())),
            None => Ok(None),
        }
    }

    /// Save current proxy state.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the state can't be encoded or written.
    pub async fn save(&self, state: &PersistedProxyState) -> Result<(), StorageError> {
        let data = serde_json::to_value(state)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let resource = ResourceState {
            id: PROXY_STATE_ID.to_string(),
            kind: "proxy_state".to_string(),
            provider: "foundation_proxy".to_string(),
            status: foundation_db::core::state::types::StateStatus::Created,
            environment: None,
            config_hash: String::new(),
            output: data,
            config_snapshot: serde_json::Value::Null,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        self.store.set(PROXY_STATE_ID, &resource).await
    }

    /// Set a backend's state string.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the state can't be loaded or saved.
    pub async fn save_backend_state(
        &self,
        service: &str,
        backend_url: &str,
        state: &str,
    ) -> Result<(), StorageError> {
        let mut persisted = self.load().await?.unwrap_or_default();
        persisted
            .backend_states
            .insert(format!("{service}/{backend_url}"), state.to_string());
        self.save(&persisted).await
    }

    /// Deterministic hash of proxy config for change detection.
    pub fn compute_config_hash(config: &ProxyConfig) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        config.domain.hash(&mut hasher);
        config.public_ip.hash(&mut hasher);
        for svc in &config.services {
            svc.name.hash(&mut hasher);
            svc.host.hash(&mut hasher);
            for be in &svc.backends {
                be.url.hash(&mut hasher);
            }
        }
        format!("{:016x}", hasher.finish())
    }

    /// Store TLS certificate data.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the state can't be loaded or saved.
    pub async fn save_tls_cert(&self, cert_pem: &str, key_pem: &str) -> Result<(), StorageError> {
        let mut state = self.load().await?.unwrap_or_default();
        state.cert_pem = Some(cert_pem.to_string());
        state.key_pem = Some(key_pem.to_string());
        state.cert_renewed_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        self.save(&state).await
    }
}

