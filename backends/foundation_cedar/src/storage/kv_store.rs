use foundation_core::valtron::Stream;
use foundation_db::traits::{KeyValueStore, StorageItemStream};
use foundation_db::StorageError;

use crate::core::errors::CedarError;
use super::traits::PolicyStore;

fn collect_first<T>(stream: StorageItemStream<'_, T>) -> Result<Option<T>, StorageError> {
    for item in stream {
        if let Stream::Next(result) = item {
            return result.map(Some);
        }
    }
    Ok(None)
}

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
        let stream = self
            .store
            .get::<String>(&key)
            .map_err(|e| CedarError::PolicyParse(format!("KV store error: {e}")))?;
        let value: Option<Option<String>> =
            collect_first(stream).map_err(|e| CedarError::PolicyParse(format!("KV read error: {e}")))?;
        value
            .flatten()
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
        let stream = self
            .store
            .get::<String>(&key)
            .map_err(|e| CedarError::PolicyParse(format!("KV store error: {e}")))?;
        let value =
            collect_first(stream).map_err(|e| CedarError::PolicyParse(format!("KV read error: {e}")))?;
        Ok(value.flatten())
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

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_db::MemoryStorage;

    fn seed_store(store: &MemoryStorage, ns: &str) {
        let schema = "entity User;".to_string();
        let policies = "permit(principal, action, resource);".to_string();
        let version = "42".to_string();

        let _ = store.set(&format!("{ns}:schema"), schema);
        let _ = store.set(&format!("{ns}:policies"), policies);
        let _ = store.set(&format!("{ns}:version"), version);
    }

    #[test]
    fn test_kv_store_loads_all() {
        let mem = MemoryStorage::new();
        seed_store(&mem, "authz");

        let store = KvPolicyStore::new(&mem, "authz");
        assert!(store.load_schema().unwrap().contains("entity User"));
        assert!(store.load_policies().unwrap().contains("permit"));
        assert!(store.load_entities().unwrap().is_none());
        assert_eq!(store.version().unwrap(), "42");
    }

    #[test]
    fn test_kv_store_with_entities() {
        let mem = MemoryStorage::new();
        seed_store(&mem, "authz");
        let _ = mem.set("authz:entities", "[]".to_string());

        let store = KvPolicyStore::new(&mem, "authz");
        assert_eq!(store.load_entities().unwrap().unwrap(), "[]");
    }

    #[test]
    fn test_kv_store_missing_key() {
        let mem = MemoryStorage::new();
        let store = KvPolicyStore::new(&mem, "missing");
        assert!(store.load_policies().is_err());
    }

    #[test]
    fn test_kv_store_namespacing() {
        let mem = MemoryStorage::new();
        seed_store(&mem, "ns1");

        let _ = mem.set("ns2:schema", "entity Admin;".to_string());
        let _ = mem.set("ns2:policies", "forbid(principal, action, resource);".to_string());
        let _ = mem.set("ns2:version", "1".to_string());

        let s1 = KvPolicyStore::new(&mem, "ns1");
        let s2 = KvPolicyStore::new(&mem, "ns2");

        assert!(s1.load_schema().unwrap().contains("User"));
        assert!(s2.load_schema().unwrap().contains("Admin"));
    }
}
