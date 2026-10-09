//! Integration tests for proxy state persistence.

use std::collections::HashMap;
use foundation_proxy::config::ProxyConfig;
use foundation_proxy::persistence::{PersistedProxyState, ProxyStateStore};

#[test]
fn persisted_state_default_is_empty() {
    let state = PersistedProxyState::default();
    assert!(state.backend_states.is_empty());
    assert!(state.cert_pem.is_none());
    assert!(state.config_hash.is_none());
}

#[test]
fn persisted_state_serializes_roundtrip() {
    let mut state = PersistedProxyState::default();
    state
        .backend_states
        .insert("svc/http://a:8080".into(), "Draining".into());
    state.config_hash = Some("deadbeef".into());
    let json = serde_json::to_string(&state).expect("serialize");
    let roundtripped: PersistedProxyState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        roundtripped.backend_states.get("svc/http://a:8080"),
        Some(&"Draining".to_string())
    );
}

#[test]
fn config_hash_changes_with_different_ip() {
    let a = ProxyConfig::new("example.com", "1.2.3.4");
    let b = ProxyConfig::new("example.com", "5.6.7.8");
    assert_ne!(
        ProxyStateStore::compute_config_hash(&a),
        ProxyStateStore::compute_config_hash(&b),
    );
}

#[test]
fn config_hash_stable_for_same_config() {
    let a = ProxyConfig::new("example.com", "1.2.3.4");
    let b = ProxyConfig::new("example.com", "1.2.3.4");
    assert_eq!(
        ProxyStateStore::compute_config_hash(&a),
        ProxyStateStore::compute_config_hash(&b),
    );
}
