use cedar_policy::Entities;

use crate::core::engine::CedarEngine;
use crate::core::errors::CedarError;
use crate::storage::traits::PolicyStore;

pub fn load_from_store(store: &dyn PolicyStore) -> Result<CedarEngine, CedarError> {
    let schema_text = store.load_schema()?;
    let policy_text = store.load_policies()?;
    let entities_json = store.load_entities()?;

    let entities = match entities_json {
        Some(ref json) => Entities::from_json_str(json, None)
            .map_err(|e| CedarError::EntityParse(e.to_string()))?,
        None => Entities::empty(),
    };

    CedarEngine::with_entities(&schema_text, &policy_text, &entities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::traits::InMemoryPolicyStore;

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

    #[test]
    fn test_load_from_store() {
        let store = InMemoryPolicyStore::new(SCHEMA.into(), POLICIES.into());
        let engine = load_from_store(&store);
        assert!(engine.is_ok());
    }

    #[test]
    fn test_load_from_store_with_entities() {
        let entities_json = r#"[
            {
                "uid": {"type":"User","id":"alice"},
                "attrs": {},
                "parents": []
            }
        ]"#;
        let store = InMemoryPolicyStore::new(SCHEMA.into(), POLICIES.into())
            .with_entities(entities_json.into());
        let engine = load_from_store(&store);
        assert!(engine.is_ok());
    }
}
