use std::path::{Path, PathBuf};

use crate::core::errors::CedarError;
use super::traits::PolicyStore;

pub struct FilePolicyStore {
    schema_path: PathBuf,
    policy_path: PathBuf,
    entities_path: Option<PathBuf>,
}

impl FilePolicyStore {
    #[must_use]
    pub fn new(schema_path: impl Into<PathBuf>, policy_path: impl Into<PathBuf>) -> Self {
        Self {
            schema_path: schema_path.into(),
            policy_path: policy_path.into(),
            entities_path: None,
        }
    }

    #[must_use]
    pub fn with_entities(mut self, path: impl Into<PathBuf>) -> Self {
        self.entities_path = Some(path.into());
        self
    }

    fn read_file(path: &Path) -> Result<String, CedarError> {
        std::fs::read_to_string(path).map_err(|e| {
            CedarError::PolicyParse(format!("Failed to read {}: {e}", path.display()))
        })
    }
}

impl PolicyStore for FilePolicyStore {
    fn load_policies(&self) -> Result<String, CedarError> {
        Self::read_file(&self.policy_path)
    }

    fn load_schema(&self) -> Result<String, CedarError> {
        Self::read_file(&self.schema_path)
    }

    fn load_entities(&self) -> Result<Option<String>, CedarError> {
        match &self.entities_path {
            Some(p) => Self::read_file(p).map(Some),
            None => Ok(None),
        }
    }

    fn version(&self) -> Result<String, CedarError> {
        let meta = std::fs::metadata(&self.policy_path).map_err(|e| {
            CedarError::PolicyParse(format!(
                "Failed to stat {}: {e}",
                self.policy_path.display()
            ))
        })?;
        let modified = meta
            .modified()
            .map_err(|e| CedarError::PolicyParse(format!("No mtime: {e}")))?;
        let dur = modified
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Ok(dur.as_secs().to_string())
    }
}

impl core::fmt::Debug for FilePolicyStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FilePolicyStore")
            .field("schema", &self.schema_path)
            .field("policies", &self.policy_path)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_file_store_loads_policies() {
        let dir = std::env::temp_dir().join("cedar_file_store_test");
        let _ = std::fs::create_dir_all(&dir);

        let schema_path = dir.join("schema.cedarschema");
        let policy_path = dir.join("policies.cedar");

        let mut sf = std::fs::File::create(&schema_path).unwrap();
        writeln!(sf, "entity User;").unwrap();

        let mut pf = std::fs::File::create(&policy_path).unwrap();
        writeln!(pf, "permit(principal, action, resource);").unwrap();

        let store = FilePolicyStore::new(&schema_path, &policy_path);
        assert!(store.load_schema().unwrap().contains("entity User"));
        assert!(store.load_policies().unwrap().contains("permit"));
        assert!(store.load_entities().unwrap().is_none());
        assert!(!store.version().unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_store_with_entities() {
        let dir = std::env::temp_dir().join("cedar_file_store_ent_test");
        let _ = std::fs::create_dir_all(&dir);

        let schema_path = dir.join("schema.cedarschema");
        let policy_path = dir.join("policies.cedar");
        let entities_path = dir.join("entities.json");

        std::fs::write(&schema_path, "entity User;").unwrap();
        std::fs::write(&policy_path, "permit(principal, action, resource);").unwrap();
        std::fs::write(&entities_path, "[]").unwrap();

        let store = FilePolicyStore::new(&schema_path, &policy_path)
            .with_entities(&entities_path);
        assert_eq!(store.load_entities().unwrap().unwrap(), "[]");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_store_missing_file() {
        let store = FilePolicyStore::new("/nonexistent/schema", "/nonexistent/policies");
        assert!(store.load_policies().is_err());
        assert!(store.load_schema().is_err());
    }
}
