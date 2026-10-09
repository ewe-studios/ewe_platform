use std::path::{Path, PathBuf};

use super::traits::PolicyStore;
use crate::core::errors::CedarError;

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
        std::fs::read_to_string(path)
            .map_err(|e| CedarError::PolicyParse(format!("Failed to read {}: {e}", path.display())))
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
