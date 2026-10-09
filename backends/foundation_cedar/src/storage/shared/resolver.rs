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
