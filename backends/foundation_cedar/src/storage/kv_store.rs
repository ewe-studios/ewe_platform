use foundation_db::traits::KeyValueStore;

use super::traits::PolicyStore;
use crate::core::errors::CedarError;

pub struct KvPolicyStore<'a, S: KeyValueStore> {
    store: &'a S,
    namespace: String,
}

impl<'a, S: KeyValueStore> KvPolicyStore<'a, S> {
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

    fn load_required(&self, suffix: &str) -> Result<String, CedarError> {
        let key = self.key(suffix);
        self.store
            .get::<String>(&key)
            .map_err(|e| CedarError::PolicyParse(format!("KV store error: {e}")))?
            .ok_or_else(|| CedarError::PolicyParse(format!("Key '{key}' not found in store")))
    }
}

impl<S: KeyValueStore> PolicyStore for KvPolicyStore<'_, S> {
    fn load_policies(&self) -> Result<String, CedarError> {
        self.load_required("policies")
    }

    fn load_schema(&self) -> Result<String, CedarError> {
        self.load_required("schema")
    }

    fn load_entities(&self) -> Result<Option<String>, CedarError> {
        let key = self.key("entities");
        self.store
            .get::<String>(&key)
            .map_err(|e| CedarError::PolicyParse(format!("KV store error: {e}")))
    }

    fn version(&self) -> Result<String, CedarError> {
        self.load_required("version")
    }
}

impl<S: KeyValueStore> core::fmt::Debug for KvPolicyStore<'_, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KvPolicyStore")
            .field("namespace", &self.namespace)
            .finish()
    }
}
