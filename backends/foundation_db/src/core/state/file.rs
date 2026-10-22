//! JSON file-based state store.
//!
//! WHY: Simplest backend — no dependencies, git-friendly, human-readable.
//! Good default for single-machine development.
//!
//! WHAT: Each resource is a separate JSON file under
//! `.deployment/{project}/{stage}/{resource_id}.json`.
//!
//! HOW: `std::fs` operations inside the async `StateStore` methods. The files
//! are small and local, so every future is ready on its first poll; listings
//! read the directory (names only) and stream the results.

use std::path::{Path, PathBuf};

use futures_lite::StreamExt;

use super::traits::{StateStore, StateStoreStream};
use super::types::ResourceState;
use crate::core::errors::{StorageError, StorageResult};

/// JSON file-based state store.
///
/// Stores each resource as a separate JSON file:
///   `.deployment/{project}/{stage}/{resource_id}.json`
///
/// Its futures complete on their first poll (local `std::fs`).
pub struct FileStateStore {
    root_dir: PathBuf,
}

impl FileStateStore {
    /// Create a new file state store.
    ///
    /// Files are stored under `{project_dir}/.deployment/{project}/{stage}/`.
    #[must_use]
    pub fn new(project_dir: &Path, project: &str, stage: &str) -> Self {
        Self {
            root_dir: project_dir.join(".deployment").join(project).join(stage),
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

    /// The `*.json` files under the root, sorted by path (empty if the root
    /// doesn't exist yet).
    fn json_files(&self) -> StorageResult<Vec<PathBuf>> {
        let mut files = Vec::new();
        if self.root_dir.exists() {
            for entry in std::fs::read_dir(&self.root_dir)? {
                let path = entry?.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
    }

    fn read_state(path: &Path) -> StorageResult<ResourceState> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(|e| StorageError::Serialization(e.to_string()))
    }

    fn id_of(path: &Path) -> Option<String> {
        path.file_stem().map(|name| name.to_string_lossy().replace(':', "/"))
    }
}

/// A stream over already-computed results.
fn stream_of<T: Send + 'static>(items: Vec<StorageResult<T>>) -> StateStoreStream<T> {
    Box::pin(futures_lite::stream::iter(items))
}

#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
impl StateStore for FileStateStore {
    async fn init(&self) -> StorageResult<()> {
        std::fs::create_dir_all(&self.root_dir)?;
        Ok(())
    }

    async fn list(&self) -> StorageResult<StateStoreStream<String>> {
        let ids = self.json_files()?.iter().filter_map(|p| Self::id_of(p)).map(Ok).collect();
        Ok(stream_of(ids))
    }

    async fn count(&self) -> StorageResult<usize> {
        Ok(self.json_files()?.len())
    }

    async fn get(&self, resource_id: &str) -> StorageResult<Option<ResourceState>> {
        let path = self.resource_path(resource_id);
        if !path.exists() {
            return Ok(None);
        }
        Self::read_state(&path).map(Some)
    }

    async fn get_batch(&self, ids: &[&str]) -> StorageResult<StateStoreStream<ResourceState>> {
        let states = ids
            .iter()
            .map(|id| self.resource_path(id))
            .filter(|path| path.exists())
            .map(|path| Self::read_state(&path))
            .collect();
        Ok(stream_of(states))
    }

    async fn all(&self) -> StorageResult<StateStoreStream<ResourceState>> {
        // Each file is read as the stream is pulled, not up front.
        let files = self.json_files()?;
        Ok(Box::pin(futures_lite::stream::iter(files).map(|path| Self::read_state(&path))))
    }

    async fn set(&self, _resource_id: &str, state: &ResourceState) -> StorageResult<()> {
        std::fs::create_dir_all(&self.root_dir)?;
        let path = self.resource_path(&state.id);
        let content = serde_json::to_string_pretty(state)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    async fn delete(&self, resource_id: &str) -> StorageResult<()> {
        let path = self.resource_path(resource_id);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

