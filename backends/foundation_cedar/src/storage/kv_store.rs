use foundation_db::AsyncKeyValueStore;

use super::traits::PolicyStore;
use crate::core::errors::CedarError;

/// Policies, schema, entities and version kept under `{namespace}:{key}` in an
/// async key-value store.
pub struct KvPolicyStore<'a, S: AsyncKeyValueStore> {
    store: &'a S,
    namespace: String,
}

impl<'a, S: AsyncKeyValueStore> KvPolicyStore<'a, S> {
    #[must_use]
    pub fn new(store: &'a S, namespace: &str) -> Self {
        Self {
            store,
            namespace: namespace.to_string(),
        }
    }

    fn key(&self, suffix: &str) -> String {
        format!("{}:{suffix}", self.namespace)
    }

    async fn load_optional(&self, suffix: &str) -> Result<Option<String>, CedarError> {
        self.store
            .get_async::<String>(&self.key(suffix))
            .await
            .map_err(|e| CedarError::PolicyParse(format!("KV store error: {e}")))
    }

    async fn load_required(&self, suffix: &str) -> Result<String, CedarError> {
        self.load_optional(suffix).await?.ok_or_else(|| {
            CedarError::PolicyParse(format!("Key '{}' not found in store", self.key(suffix)))
        })
    }
}

#[async_trait::async_trait]
impl<S: AsyncKeyValueStore> PolicyStore for KvPolicyStore<'_, S> {
    async fn load_policies(&self) -> Result<String, CedarError> {
        self.load_required("policies").await
    }

    async fn load_schema(&self) -> Result<String, CedarError> {
        self.load_required("schema").await
    }

    async fn load_entities(&self) -> Result<Option<String>, CedarError> {
        self.load_optional("entities").await
    }

    async fn version(&self) -> Result<String, CedarError> {
        self.load_required("version").await
    }
}

impl<S: AsyncKeyValueStore> core::fmt::Debug for KvPolicyStore<'_, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KvPolicyStore")
            .field("namespace", &self.namespace)
            .finish()
    }
}
