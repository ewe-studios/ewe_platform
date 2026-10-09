use foundation_cedar::{InMemoryPolicyStore, PolicyStore};

#[test]
fn test_in_memory_store() {
    let store = InMemoryPolicyStore::new(
        "entity User;".into(),
        r#"permit(principal, action, resource);"#.into(),
    );
    assert_eq!(store.load_schema().unwrap(), "entity User;");
    assert!(store.load_policies().unwrap().contains("permit"));
    assert!(store.load_entities().unwrap().is_none());
    assert_eq!(store.version().unwrap(), "1");
}
