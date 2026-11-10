//! JSON file-based state store.
//!
//! WHY: Simplest backend — no dependencies, git-friendly, human-readable.
//! Good default for single-machine development.
//!
//! WHAT: Each resource is a separate JSON file under
//! `.deployment/{project}/{stage}/{resource_id}.json`.
//!
//! HOW: `set` writes a temp file, syncs it and renames it over the target, so
//! a reader never sees a torn file. `std::fs` operations inside the async `StateStore` methods. The files
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
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(content)?;
            file.sync_all()?;
            drop(file);
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

    fn id_of(path: &Path) -> Option<String> {
        path.file_stem()
            .map(|name| name.to_string_lossy().replace(':', "/"))
    }
}

/// A stream over already-computed results.
fn stream_of<T: Send + 'static>(items: Vec<StorageResult<T>>) -> StateStoreStream<T> {
    Box::pin(futures_lite::stream::iter(items))
}

/// An exclusive lock on a file, released when dropped.
struct LockGuard {
    // Held for the lock's life: closing the file releases the `flock`.
    _file: std::fs::File,
    _process: std::sync::MutexGuard<'static, ()>,
}

impl LockGuard {
    fn lock(file: std::fs::File) -> StorageResult<Self> {
        // One lock per process too: `flock` on two handles of one file in one
        // process also excludes, but this makes it hold on every platform.
        static PROCESS: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let process = PROCESS
            .lock()
            .map_err(|e| StorageError::Backend(format!("lock poisoned: {e}")))?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: `fd` is a valid open file descriptor owned by `file`.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
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
        let ids = self
            .json_files()?
            .iter()
            .filter_map(|p| Self::id_of(p))
            .map(Ok)
            .collect();
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
        Ok(Box::pin(
            futures_lite::stream::iter(files).map(|path| Self::read_state(&path)),
        ))
    }

    async fn set(&self, resource_id: &str, state: &ResourceState) -> StorageResult<()> {
        std::fs::create_dir_all(&self.root_dir)?;
        let state = state.with_id(resource_id);
        let path = self.resource_path(&state.id);
        let content = serde_json::to_string_pretty(&state)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Self::write_atomically(&path, content.as_bytes())?;
        Ok(())
    }

    async fn set_if_version(
        &self,
        resource_id: &str,
        state: &ResourceState,
        expected: Option<chrono::DateTime<chrono::Utc>>,
    ) -> StorageResult<bool> {
        std::fs::create_dir_all(&self.root_dir)?;
        // An exclusive `flock` on a lock file covers read, compare and rename,
        // so writers in other threads and other processes take turns. (On a
        // platform without `flock`, only writers in this process take turns.)
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.root_dir.join(".state.lock"))?;
        let _lock = LockGuard::lock(lock)?;
        let path = self.resource_path(resource_id);
        let current = if path.exists() {
            Some(Self::read_state(&path)?.updated_at)
        } else {
            None
        };
        if current != expected {
            return Ok(false);
        }
        let state = state.with_id(resource_id);
        let content = serde_json::to_string_pretty(&state)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Self::write_atomically(&path, content.as_bytes())?;
        Ok(true)
    }

    async fn delete(&self, resource_id: &str) -> StorageResult<()> {
        let path = self.resource_path(resource_id);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}
