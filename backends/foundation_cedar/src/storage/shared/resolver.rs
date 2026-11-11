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
    let bundle = store.load_all().await?;

    let entities = match bundle.entities {
        Some(ref json) => Entities::from_json_str(json, None)
            .map_err(|e| CedarError::EntityParse(e.to_string()))?,
        None => Entities::empty(),
    };

    CedarEngine::with_entities(&bundle.schema, &bundle.policies, &entities)
}
