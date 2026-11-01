use super::super::core::errors::CedarError;

/// The four policy documents a [`PolicyStore`] can load, in one bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyBundle {
    pub schema: String,
    pub policies: String,
    pub entities: Option<String>,
    pub version: String,
}

/// A source of Cedar policies, schema and entities.
///
/// Async because the storage-backed sources (`KvPolicyStore`,
/// `SqlPolicyStore`) read through the async `foundation_db` stores.
#[async_trait::async_trait]
pub trait PolicyStore: core::fmt::Debug + Send + Sync {
    async fn load_policies(&self) -> Result<String, CedarError>;
    async fn load_schema(&self) -> Result<String, CedarError>;
    async fn load_entities(&self) -> Result<Option<String>, CedarError> {
        Ok(None)
    }
    async fn version(&self) -> Result<String, CedarError>;

    /// Load the schema, policies, entities and version together.
    ///
    /// The default implementation runs the four per-field loads. A store that
    /// keeps all four values in one record (for example `SqlPolicyStore`)
    /// overrides this with a single read.
    async fn load_all(&self) -> Result<PolicyBundle, CedarError> {
        Ok(PolicyBundle {
            schema: self.load_schema().await?,
            policies: self.load_policies().await?,
            entities: self.load_entities().await?,
            version: self.version().await?,
        })
    }
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

#[async_trait::async_trait]
impl PolicyStore for InMemoryPolicyStore {
    async fn load_policies(&self) -> Result<String, CedarError> {
        Ok(self.policies.clone())
    }

    async fn load_schema(&self) -> Result<String, CedarError> {
        Ok(self.schema.clone())
    }

    async fn load_entities(&self) -> Result<Option<String>, CedarError> {
        Ok(self.entities.clone())
    }

    async fn version(&self) -> Result<String, CedarError> {
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
