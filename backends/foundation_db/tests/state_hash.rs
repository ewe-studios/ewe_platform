use foundation_db::state::config_hash;
use serde_json::json;

#[test]
fn deterministic_hash() {
    let config = json!({"name": "my-worker", "account_id": "abc123"});
    let h1 = config_hash(&config).unwrap();
    let h2 = config_hash(&config).unwrap();
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

#[test]
fn different_configs_different_hashes() {
    let a = json!({"name": "worker-a"});
    let b = json!({"name": "worker-b"});
    assert_ne!(config_hash(&a).unwrap(), config_hash(&b).unwrap());
}

#[test]
fn empty_object_hashes() {
    let empty = json!({});
    let h = config_hash(&empty).unwrap();
    assert_eq!(h.len(), 64);
}
