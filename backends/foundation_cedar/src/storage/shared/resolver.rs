use cedar_policy::Entities;

use crate::core::engine::CedarEngine;
use crate::core::errors::CedarError;
use crate::storage::traits::PolicyStore;

/// Build a [`CedarEngine`] from a policy source.
///
/// # Errors
///
/// Returns [`CedarError`] if the source can't be read, or its schema,
/// policies or entities don't parse.
pub async fn load_from_store(store: &dyn PolicyStore) -> Result<CedarEngine, CedarError> {
    let schema_text = store.load_schema().await?;
    let policy_text = store.load_policies().await?;
    let entities_json = store.load_entities().await?;

    let entities = match entities_json {
        Some(ref json) => Entities::from_json_str(json, None)
            .map_err(|e| CedarError::EntityParse(e.to_string()))?,
        None => Entities::empty(),
    };

    CedarEngine::with_entities(&schema_text, &policy_text, &entities)
}
