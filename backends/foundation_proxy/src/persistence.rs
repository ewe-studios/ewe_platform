//! Proxy state persistence (Decision 21).
//!
//! WHY: The proxy must survive restarts without losing backend drain/pause
//! state, TLS certificate data, or deployment configuration. Wraps
//! `foundation_db::FileStateStore` for simple JSON-file persistence.
//!
//! WHAT: [`ProxyStateStore`] — load/save backend states and TLS cert data
//! across proxy restarts. Keyed by service name + backend URL.

use std::collections::HashMap;
use std::path::Path;

use foundation_db::core::state::{FileStateStore, StateStore};
use foundation_db::core::errors::StorageError;
use serde::{Deserialize, Serialize};

use crate::config::ProxyConfig;

/// Persistent proxy state — survives restarts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedProxyState {
    /// Backend states: "service_name/backend_url" → backend_state
    pub backend_states: HashMap<String, String>,
    /// Certificate data for TLS provisioning (PEM-encoded cert + key)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert_pem: Option<String>,
    /// Private key (PEM)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_pem: Option<String>,
    /// Timestamp of last certificate renewal (unix seconds)
    pub cert_renewed_at: Option<u64>,
    /// Configuration hash — compared on startup to detect config changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_hash: Option<String>,
}

/// Wraps `FileStateStore` for proxy-specific persistence operations.
pub struct ProxyStateStore {
    store: FileStateStore,
}

impl ProxyStateStore {
    /// Create a store under `state_dir` for the given proxy domain.
    pub fn new(state_dir: &Path, domain: &str) -> Self {
        Self {
            store: FileStateStore::new(state_dir, "foundation_proxy", domain),
        }
    }

    /// Load persisted proxy state, if any exists.
    pub fn load(&self) -> Result<Option<PersistedProxyState>, StorageError> {
        match self.store.load_typed::<PersistedProxyState>("proxy_state") {
            Ok(v) => Ok(Some(v)),
            Err(StorageError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Save current proxy state.
    pub fn save(&self, state: &PersistedProxyState) -> Result<(), StorageError> {
        self.store.store_typed("proxy_state", state)
    }

    /// Set a backend's state string (e.g. "Active", "Draining", "Paused").
    pub fn save_backend_state(
        &self,
        service: &str,
        backend_url: &str,
        state: &str,
    ) -> Result<(), StorageError> {
        let mut persisted = self.load()?.unwrap_or_default();
        persisted
            .backend_states
            .insert(format!("{service}/{backend_url}"), state.to_string());
        self.save(&persisted)
    }

    /// Compute a simple hash of the current proxy config for change detection.
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

    /// Store TLS certificate data for persistence across restarts.
    pub fn save_tls_cert(
        &self,
        cert_pem: &str,
        key_pem: &str,
    ) -> Result<(), StorageError> {
        let mut state = self.load()?.unwrap_or_default();
        state.cert_pem = Some(cert_pem.to_string());
        state.key_pem = Some(key_pem.to_string());
        state.cert_renewed_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        self.save(&state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_state_default_is_empty() {
        let state = PersistedProxyState::default();
        assert!(state.backend_states.is_empty());
        assert!(state.cert_pem.is_none());
        assert!(state.config_hash.is_none());
    }

    #[test]
    fn persisted_state_serializes_roundtrip() {
        let state = PersistedProxyState {
            backend_states: HashMap::from([
                ("svc/http://a:8080".to_string(), "Draining".to_string()),
            ]),
            cert_pem: None,
            key_pem: None,
            cert_renewed_at: None,
            config_hash: Some("deadbeef".to_string()),
        };
        let json = serde_json::to_string(&state).expect("serialize");
        let roundtripped: PersistedProxyState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            roundtripped.backend_states.get("svc/http://a:8080"),
            Some(&"Draining".to_string())
        );
    }

    #[test]
    fn config_hash_changes_with_different_backend() {
        let a = ProxyConfig::new("example.com", "1.2.3.4");
        let b = ProxyConfig::new("example.com", "5.6.7.8");
        assert_ne!(
            ProxyStateStore::compute_config_hash(&a),
            ProxyStateStore::compute_config_hash(&b),
            "different public IPs should produce different hashes"
        );
    }

    #[test]
    fn config_hash_stable_for_same_config() {
        let a = ProxyConfig::new("example.com", "1.2.3.4");
        let b = ProxyConfig::new("example.com", "1.2.3.4");
        assert_eq!(
            ProxyStateStore::compute_config_hash(&a),
            ProxyStateStore::compute_config_hash(&b),
            "identical configs should produce the same hash"
        );
    }
}
