//! The record store on disk (feature 10, section 6.6).
//!
//! One JSON file for each record in `<state_dir>/records/`. A write goes to a
//! temporary file first and then to a rename, so a crash never leaves a half
//! record: the old file stays until the new one is complete (feature 10 test
//! `file_store_atomic`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::shared::control::record::RecordStore;
use crate::shared::driver::StoredRecord;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::HostFs;
use crate::shared::ids::SandboxId;
use crate::shared::BoxFuture;

/// The counter that makes every temporary file name unique.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A record store of one JSON file for each record.
pub struct FileRecordStore {
    dir: PathBuf,
    fs: Arc<dyn HostFs>,
    max_record_bytes: usize,
}

impl std::fmt::Debug for FileRecordStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileRecordStore")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl FileRecordStore {
    /// A store under `<state_dir>/records/`.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the directory cannot be made.
    pub fn new(fs: Arc<dyn HostFs>, state_dir: impl Into<PathBuf>) -> SandboxResult<Self> {
        let dir = state_dir.into().join("records");
        fs.create_dir_all(&dir, 0o700)?;
        Ok(Self {
            dir,
            fs,
            max_record_bytes: 1024 * 1024,
        })
    }

    /// Replaces the largest record that the store reads.
    ///
    /// Default 1 MiB: a record holds one spec and its endpoint, so this is a
    /// generous cap on a corrupt or hostile file.
    #[must_use]
    pub fn with_max_record_bytes(mut self, bytes: usize) -> Self {
        self.max_record_bytes = bytes;
        self
    }

    /// The directory of the records.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file of one record.
    #[must_use]
    pub fn record_path(&self, id: &SandboxId) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// The temporary file of one write.
    #[must_use]
    pub fn temp_path(&self, id: &SandboxId) -> PathBuf {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        self.dir
            .join(format!("{id}.{}.{counter}.json.tmp", std::process::id()))
    }

    /// Writes the record to a temporary file and gives its path.
    ///
    /// A caller that wants to prove the atomic write calls this method alone:
    /// the old record is still the one that [`RecordStore::list`] reads.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be written.
    pub fn stage(&self, record: &StoredRecord) -> SandboxResult<PathBuf> {
        let bytes = serde_json::to_vec(record).map_err(|error| SandboxError::Io {
            op: "serialize record".to_string(),
            message: error.to_string(),
        })?;
        let temp = self.temp_path(&record.id);
        self.fs.write(&temp, &bytes)?;
        Ok(temp)
    }

    /// Renames a staged record into place.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the rename fails.
    pub fn commit(&self, id: &SandboxId, temp: &Path) -> SandboxResult<()> {
        self.fs.rename(temp, &self.record_path(id))
    }
}

impl RecordStore for FileRecordStore {
    fn put(&self, record: StoredRecord) -> BoxFuture<'_, SandboxResult<()>> {
        let temp = match self.stage(&record) {
            Ok(temp) => temp,
            Err(error) => return Box::pin(async move { Err(error) }),
        };
        let id = record.id;
        let result = self.commit(&id, &temp);
        Box::pin(async move { result })
    }

    fn remove(&self, id: &SandboxId) -> BoxFuture<'_, SandboxResult<()>> {
        let path = self.record_path(id);
        let fs = Arc::clone(&self.fs);
        Box::pin(async move { fs.remove_file(&path) })
    }

    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<StoredRecord>>> {
        let entries = match self.fs.read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) => return Box::pin(async move { Err(error) }),
        };
        let mut records = Vec::new();
        for path in entries {
            // A temporary file is not a record; a crash leaves one behind.
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
                continue;
            }
            let bytes = match self.fs.read(&path, self.max_record_bytes) {
                Ok(bytes) => bytes,
                Err(error) => return Box::pin(async move { Err(error) }),
            };
            match serde_json::from_slice::<StoredRecord>(&bytes) {
                Ok(record) => records.push(record),
                Err(error) => {
                    let error = SandboxError::Io {
                        op: "read record".to_string(),
                        message: format!("{}: {error}", path.display()),
                    };
                    return Box::pin(async move { Err(error.into()) });
                }
            }
        }
        Box::pin(async move { Ok(records) })
    }
}
