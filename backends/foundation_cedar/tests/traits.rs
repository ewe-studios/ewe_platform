use foundation_cedar::{InMemoryPolicyStore, PolicyStore};
use foundation_core::valtron::valtron_test;

#[valtron_test]
async fn test_in_memory_store() {
    let store = InMemoryPolicyStore::new(
        "entity User;".into(),
        r#"permit(principal, action, resource);"#.into(),
    );
    assert_eq!(store.load_schema().await.unwrap(), "entity User;");
    assert!(store.load_policies().await.unwrap().contains("permit"));
    assert!(store.load_entities().await.unwrap().is_none());
    assert_eq!(store.version().await.unwrap(), "1");
}
