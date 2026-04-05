//! JSON file-based state store.
//!
//! WHY: Simplest backend — no dependencies, git-friendly, human-readable.
//! Good default for single-machine development.
//!
//! WHAT: Each resource is a separate JSON file under
//! `.deployment/{provider}/{stage}/{resource_id}.json`.
//!
//! HOW: Purely synchronous `std::fs` operations. Returns `StateStoreStream`
//! built from `Vec::into_iter().map(...)` for trait consistency. No Valtron.

use std::path::{Path, PathBuf};

use foundation_core::valtron::ThreadedValue;

use crate::errors::StorageError;
use super::traits::{StateStore, StateStoreStream};
use super::types::ResourceState;

/// JSON file-based state store.
///
/// Stores each resource as a separate JSON file:
///   `.deployment/{provider}/{stage}/{resource_id}.json`
///
/// Purely synchronous — no Valtron needed.
pub struct FileStateStore {
    root_dir: PathBuf,
}

impl FileStateStore {
    /// Create a new file state store.
    ///
    /// Files are stored under `{project_dir}/.deployment/{provider}/{stage}/`.
    #[must_use]
    pub fn new(project_dir: &Path, provider: &str, stage: &str) -> Self {
        Self {
            root_dir: project_dir
                .join(".deployment")
                .join(provider)
                .join(stage),
        }
    }

    /// Create a file state store with an explicit root directory.
    #[must_use]
    pub fn with_root(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    fn resource_path(&self, resource_id: &str) -> PathBuf {
        let safe_id = resource_id.replace('/', ":");
        self.root_dir.join(format!("{safe_id}.json"))
    }

    /// Wrap a single value into a `StateStoreStream`.
    fn wrap_value<T: Send + 'static>(val: T) -> StateStoreStream<T> {
        Box::new(std::iter::once(ThreadedValue::Value(Ok(val))))
    }

    /// Wrap a `Vec` into a `StateStoreStream`.
    fn wrap_vec<T: Send + 'static>(vals: Vec<T>) -> StateStoreStream<T> {
        Box::new(vals.into_iter().map(|v| ThreadedValue::Value(Ok(v))))
    }
}

impl StateStore for FileStateStore {
    fn init(&self) -> Result<(), StorageError> {
        std::fs::create_dir_all(&self.root_dir)?;
        Ok(())
    }

    fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
        let mut ids = Vec::new();
        if self.root_dir.exists() {
            for entry in std::fs::read_dir(&self.root_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    if let Some(name) = path.file_stem() {
                        ids.push(name.to_string_lossy().replace(':', "/"));
                    }
                }
            }
        }
        ids.sort();
        Ok(Self::wrap_vec(ids))
    }

    fn count(&self) -> Result<StateStoreStream<usize>, StorageError> {
        let mut count = 0;
        if self.root_dir.exists() {
            for entry in std::fs::read_dir(&self.root_dir)? {
                let entry = entry?;
                if entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "json")
                {
                    count += 1;
                }
            }
        }
        Ok(Self::wrap_value(count))
    }

    fn get(
        &self,
        resource_id: &str,
    ) -> Result<StateStoreStream<Option<ResourceState>>, StorageError> {
        let path = self.resource_path(resource_id);
        if !path.exists() {
            return Ok(Self::wrap_value(None));
        }
        let content = std::fs::read_to_string(&path)?;
        let state: ResourceState = serde_json::from_str(&content)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Ok(Self::wrap_value(Some(state)))
    }

    fn get_batch(&self, ids: &[&str]) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let mut results = Vec::new();
        for id in ids {
            let path = self.resource_path(id);
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                let state: ResourceState = serde_json::from_str(&content)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                results.push(state);
            }
        }
        Ok(Self::wrap_vec(results))
    }

    fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let mut results = Vec::new();
        if self.root_dir.exists() {
            for entry in std::fs::read_dir(&self.root_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    let content = std::fs::read_to_string(&path)?;
                    let state: ResourceState = serde_json::from_str(&content)
                        .map_err(|e| StorageError::Serialization(e.to_string()))?;
                    results.push(state);
                }
            }
        }
        results.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self::wrap_vec(results))
    }

    fn set(
        &self,
        _resource_id: &str,
        state: &ResourceState,
    ) -> Result<StateStoreStream<()>, StorageError> {
        self.init()?;
        let path = self.resource_path(&state.id);
        let content = serde_json::to_string_pretty(state)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(Self::wrap_value(()))
    }

    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, StorageError> {
        let path = self.resource_path(resource_id);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(Self::wrap_value(()))
    }
}
