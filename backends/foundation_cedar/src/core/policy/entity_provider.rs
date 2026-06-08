use cedar_policy::Entities;

use super::super::errors::CedarError;
use super::super::request::CedarRequest;

pub trait EntityProvider: Send + Sync {
    fn get_entities(&self, request: &CedarRequest) -> Result<Entities, CedarError>;
}

pub struct StaticEntityProvider {
    entities: Entities,
}

impl StaticEntityProvider {
    #[must_use]
    pub fn new(entities: Entities) -> Self {
        Self { entities }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self {
            entities: Entities::empty(),
        }
    }
}

impl EntityProvider for StaticEntityProvider {
    fn get_entities(&self, _request: &CedarRequest) -> Result<Entities, CedarError> {
        Ok(self.entities.clone())
    }
}

impl core::fmt::Debug for StaticEntityProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StaticEntityProvider").finish()
    }
}

pub struct JsonEntityProvider {
    json: String,
}

impl JsonEntityProvider {
    #[must_use]
    pub fn new(json: String) -> Self {
        Self { json }
    }
}

impl EntityProvider for JsonEntityProvider {
    fn get_entities(&self, _request: &CedarRequest) -> Result<Entities, CedarError> {
        Entities::from_json_str(&self.json, None)
            .map_err(|e| CedarError::EntityParse(e.to_string()))
    }
}

impl core::fmt::Debug for JsonEntityProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JsonEntityProvider")
            .field("json_len", &self.json.len())
            .finish()
    }
}
