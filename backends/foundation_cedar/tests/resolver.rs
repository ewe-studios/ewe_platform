use foundation_cedar::{load_from_store, InMemoryPolicyStore};
use foundation_core::valtron::valtron_test;

const SCHEMA: &str = r#"
entity User;
entity Album;
action view appliesTo {
  principal: [User],
  resource: [Album]
};
"#;

const POLICIES: &str = r#"
permit(
    principal == User::"alice",
    action == Action::"view",
    resource == Album::"trip"
);
"#;

#[valtron_test]
async fn test_load_from_store() {
    let store = InMemoryPolicyStore::new(SCHEMA.into(), POLICIES.into());
    let engine = load_from_store(&store).await;
    assert!(engine.is_ok());
}

#[valtron_test]
async fn test_load_from_store_with_entities() {
    let entities_json = r#"[
            {
                "uid": {"type":"User","id":"alice"},
                "attrs": {},
                "parents": []
            }
        ]"#;
    let store = InMemoryPolicyStore::new(SCHEMA.into(), POLICIES.into())
        .with_entities(entities_json.into());
    let engine = load_from_store(&store).await;
    assert!(engine.is_ok());
}
