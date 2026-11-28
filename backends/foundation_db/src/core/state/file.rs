//! JSON file-based state store.
//!
//! WHY: Simplest backend — no dependencies, git-friendly, human-readable.
//! Good default for single-machine development.
//!
//! WHAT: Each resource is a separate JSON file under
//! `.deployment/{project}/{stage}/resources-v1/{digest}.json`.
//! The file stores the original resource identifier.
//!
//! HOW: `set` writes a temp file, syncs it and renames it over the target, so
//! a reader never sees a torn file. `std::fs` operations inside the async `StateStore` methods. The files
//! are small and local. Listings read the resource identifiers and sort them.

use std::path::{Path, PathBuf};

use futures_lite::StreamExt;
use sha2::{Digest, Sha256};

use super::traits::{StateStore, StateStoreStream};
use super::types::ResourceState;
use crate::core::errors::{StorageError, StorageResult};

/// JSON file-based state store.
///
/// Stores each resource as a separate JSON file:
///   `.deployment/{project}/{stage}/resources-v1/{digest}.json`.
/// The digest uses the 256-bit Secure Hash Algorithm (SHA-256).
/// Existing files in the root directory remain available.
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

    fn resources_dir(&self) -> PathBuf {
        self.root_dir.join("resources-v1")
    }

    fn resource_path(&self, resource_id: &str) -> PathBuf {
        let digest = hex::encode(Sha256::digest(resource_id.as_bytes()));
        self.resources_dir().join(format!("{digest}.json"))
    }

    fn read_if_present(path: &Path) -> StorageResult<Option<ResourceState>> {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        Self::decode(&content).map(Some)
    }

    fn find(&self, resource_id: &str) -> StorageResult<Option<(PathBuf, ResourceState)>> {
        let current = self.resource_path(resource_id);
        if let Some(state) = Self::read_if_present(&current)? {
            if state.id != resource_id {
                return Err(StorageError::Backend(
                    "Resource identifier does not match its file name".into(),
                ));
            }
            return Ok(Some((current, state)));
        }

        // Use only names that the directory contains. A legacy name can contain
        // an invalid path character or name a device on another platform.
        let legacy_name = format!("{}.json", resource_id.replace('/', ":"));
        let entries = match std::fs::read_dir(&self.root_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            if entry.file_name() == legacy_name.as_str() && entry.file_type()?.is_file() {
                let path = entry.path();
                return Ok(Self::read_if_present(&path)?
                    .filter(|state| state.id == resource_id)
                    .map(|state| (path, state)));
            }
        }
        Ok(None)
    }

    /// The JSON files in both the current and legacy directories.
    fn json_files(&self) -> StorageResult<Vec<PathBuf>> {
        let mut files = Vec::new();
        for dir in [&self.root_dir, &self.resources_dir()] {
            let entries = match std::fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                if entry.file_type()?.is_file() && path.extension().is_some_and(|ext| ext == "json")
                {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
    }

    fn read_state(path: &Path) -> StorageResult<ResourceState> {
        Self::decode(&std::fs::read_to_string(path)?)
    }

    fn decode(content: &str) -> StorageResult<ResourceState> {
        serde_json::from_str(content)
            .map_err(|error| StorageError::Serialization(error.to_string()))
    }

    fn open_lock(&self) -> StorageResult<LockGuard> {
        std::fs::create_dir_all(self.resources_dir())?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root_dir.join(".state.lock"))?;
        LockGuard::lock(lock)
    }

    fn write_state(path: &Path, state: &ResourceState) -> StorageResult<()> {
        let content = serde_json::to_string_pretty(state)
            .map_err(|error| StorageError::Serialization(error.to_string()))?;
        Self::write_atomically(path, content.as_bytes())
    }

    /// Replace `path` with `content` so that a reader sees the whole old file
    /// or the whole new one, never a part: write a temp file in the same
    /// directory (so the rename stays on one file system), `fsync` it, then
    /// `rename` it over the target (atomic on POSIX). The temp name does not end
    /// in `.json`, so listings never show it. A failed write removes the temp
    /// file.
    fn write_atomically(path: &Path, content: &[u8]) -> StorageResult<()> {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let name = path
            .file_name()
            .map_or_default(|n| n.to_string_lossy().into_owned());
        let temp = dir.join(format!(
            ".{name}.{}.{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));

        let written = (|| -> std::io::Result<()> {
            {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temp)?;
                file.write_all(content)?;
                file.sync_all()?;
            }
            std::fs::rename(&temp, path)
        })();
        if let Err(e) = written {
            let _ = std::fs::remove_file(&temp);
            return Err(e.into());
        }

        // Make the rename itself durable: fsync the directory (Unix only; other
        // platforms have no directory handle to sync).
        #[cfg(unix)]
        std::fs::File::open(dir)?.sync_all()?;
        Ok(())
    }
}

/// A stream over already-computed results.
fn stream_of<T: Send + 'static>(items: Vec<StorageResult<T>>) -> StateStoreStream<T> {
    Box::pin(futures_lite::stream::iter(items))
}

/// An exclusive lock on a file, released when dropped.
struct LockGuard {
    // Closing the file releases the operating system lock.
    _file: std::fs::File,
    _process: std::sync::MutexGuard<'static, ()>,
}

impl LockGuard {
    fn lock(file: std::fs::File) -> StorageResult<Self> {
        static PROCESS: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let process = PROCESS
            .lock()
            .map_err(|e| StorageError::Backend(format!("lock poisoned: {e}")))?;
        #[cfg(any(unix, windows))]
        file.lock()?;
        Ok(Self {
            _file: file,
            _process: process,
        })
    }
}

#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
impl StateStore for FileStateStore {
    async fn init(&self) -> StorageResult<()> {
        std::fs::create_dir_all(&self.root_dir)?;
        Ok(())
    }

    async fn list(&self) -> StorageResult<StateStoreStream<String>> {
        let mut ids = self
            .json_files()?
            .into_iter()
            .filter_map(|path| {
                Self::read_if_present(&path)
                    .map(|state| state.map(|state| state.id))
                    .transpose()
            })
            .collect::<StorageResult<Vec<_>>>()?;
        ids.sort();
        Ok(stream_of(ids.into_iter().map(Ok).collect()))
    }

    async fn count(&self) -> StorageResult<usize> {
        Ok(self.json_files()?.len())
    }

    async fn get(&self, resource_id: &str) -> StorageResult<Option<ResourceState>> {
        Ok(self.find(resource_id)?.map(|(_, state)| state))
    }

    async fn get_batch(&self, ids: &[&str]) -> StorageResult<StateStoreStream<ResourceState>> {
        let states = ids
            .iter()
            .filter_map(|id| {
                self.find(id)
                    .map(|entry| entry.map(|(_, state)| state))
                    .transpose()
            })
            .collect();
        Ok(stream_of(states))
    }

    async fn all(&self) -> StorageResult<StateStoreStream<ResourceState>> {
        // Each file is read as the stream is pulled, not up front.
        let files = self.json_files()?;
        Ok(Box::pin(
            futures_lite::stream::iter(files).map(|path| Self::read_state(&path)),
        ))
    }

    async fn set(&self, resource_id: &str, state: &ResourceState) -> StorageResult<()> {
        let _lock = self.open_lock()?;
        let path = self
            .find(resource_id)?
            .map_or_else(|| self.resource_path(resource_id), |(path, _)| path);
        Self::write_state(&path, &state.with_id(resource_id))
    }

    async fn set_if_version(
        &self,
        resource_id: &str,
        state: &ResourceState,
        expected: Option<chrono::DateTime<chrono::Utc>>,
    ) -> StorageResult<bool> {
        let _lock = self.open_lock()?;
        let (path, current) = self.find(resource_id)?.map_or_else(
            || (self.resource_path(resource_id), None),
            |(path, state)| (path, Some(state.updated_at)),
        );
        if current != expected {
            return Ok(false);
        }
        Self::write_state(&path, &state.with_id(resource_id))?;
        Ok(true)
    }

    async fn delete(&self, resource_id: &str) -> StorageResult<()> {
        let _lock = self.open_lock()?;
        if let Some((path, _)) = self.find(resource_id)? {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}
