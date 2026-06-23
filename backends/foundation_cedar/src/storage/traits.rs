use super::super::core::errors::CedarError;

pub trait PolicyStore: core::fmt::Debug + Send + Sync {
    fn load_policies(&self) -> Result<String, CedarError>;
    fn load_schema(&self) -> Result<String, CedarError>;
    fn load_entities(&self) -> Result<Option<String>, CedarError> {
        Ok(None)
    }
    fn version(&self) -> Result<String, CedarError>;
}

pub struct InMemoryPolicyStore {
    schema: String,
    policies: String,
    entities: Option<String>,
    version: String,
}

impl InMemoryPolicyStore {
    #[must_use]
    pub fn new(schema: String, policies: String) -> Self {
        Self {
            schema,
            policies,
            entities: None,
            version: "1".into(),
        }
    }

    #[must_use]
    pub fn with_entities(mut self, entities: String) -> Self {
        self.entities = Some(entities);
        self
    }

    #[must_use]
    pub fn with_version(mut self, version: String) -> Self {
        self.version = version;
        self
    }
}

impl PolicyStore for InMemoryPolicyStore {
    fn load_policies(&self) -> Result<String, CedarError> {
        Ok(self.policies.clone())
    }

    fn load_schema(&self) -> Result<String, CedarError> {
        Ok(self.schema.clone())
    }

    fn load_entities(&self) -> Result<Option<String>, CedarError> {
        Ok(self.entities.clone())
    }

    fn version(&self) -> Result<String, CedarError> {
        Ok(self.version.clone())
    }
}

impl core::fmt::Debug for InMemoryPolicyStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InMemoryPolicyStore")
            .field("version", &self.version)
            .finish()
    }
}
