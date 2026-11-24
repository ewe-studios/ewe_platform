//! Open a native key-value database and read records from an older application file.
use foundation_context::Ctx;
use foundation_db::{LibsqlStore, StorageError, StorageResult};
use futures_util::lock::Mutex;
use serde::de::DeserializeOwned;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// A database for one application namespace. The application supplies its private file paths.
pub struct NativeRecordFile {
    database: PathBuf,
    legacy: PathBuf,
    legacy_limit: u64,
    lock_timeout: Duration,
    opened: Mutex<Option<Arc<LibsqlStore>>>,
}

impl NativeRecordFile {
    /// Limit old files to one mebibyte, the default nginx request body limit.
    /// Wait five seconds for other writers, the Python sqlite3 default.
    #[must_use]
    pub fn new(database: PathBuf, legacy: PathBuf) -> Self {
        Self {
            database,
            legacy,
            legacy_limit: 1024 * 1024,
            lock_timeout: Duration::from_secs(5),
            opened: Mutex::new(None),
        }
    }

    /// Set the maximum number of bytes to read from the old file.
    #[must_use]
    pub fn with_legacy_limit(mut self, bytes: u64) -> Self {
        self.legacy_limit = bytes;
        self
    }

    /// Set the time the database waits for another connection to release a write lock.
    #[must_use]
    pub fn with_lock_timeout(mut self, timeout: Duration) -> Self {
        self.lock_timeout = timeout;
        self
    }

    /// Open the database and create its key-value table on first use.
    ///
    /// # Errors
    /// Return directory, database, or schema errors.
    pub async fn database(&self, ctx: &Ctx) -> StorageResult<Arc<LibsqlStore>> {
        let mut opened = self.opened.lock().await;
        if let Some(store) = opened.as_ref() {
            return Ok(store.clone());
        }
        let path = self.database.clone();
        let path = blocking::unblock(move || {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)?;
            }
            path.into_os_string().into_string().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid database path")
            })
        })
        .await
        .map_err(StorageError::Io)?;
        // Keep the host clock without retaining the context entry that owns this file.
        let store = Arc::new(LibsqlStore::new_kv(&path, None, ctx.with_empty_map()).await?);
        store.set_busy_timeout(self.lock_timeout).await?;
        store.init_kv().await?;
        *opened = Some(store.clone());
        Ok(store)
    }

    /// Read one old JSON record without changing its file.
    ///
    /// # Errors
    /// Return an error for unreadable, invalid, or oversized data.
    pub async fn legacy_record<V: DeserializeOwned + Send + 'static>(
        &self,
    ) -> StorageResult<Option<V>> {
        let path = self.legacy.clone();
        let limit = self.legacy_limit;
        blocking::unblock(move || {
            let file = match std::fs::File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(StorageError::Io(error)),
            };
            let size = file.metadata().map_err(StorageError::Io)?.len();
            if size == 0 {
                return Ok(None);
            }
            let too_large =
                || StorageError::Migration(format!("Legacy record exceeds {limit} bytes"));
            if size > limit {
                return Err(too_large());
            }
            let mut input = BufReader::new(file).take(limit.saturating_add(1));
            let result = serde_json::from_reader(&mut input);
            if input.limit() == 0 {
                return Err(too_large());
            }
            result.map(Some).map_err(StorageError::Json)
        })
        .await
    }
}
