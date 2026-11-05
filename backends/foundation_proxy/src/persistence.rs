//! Proxy state persistence (Decision 21).
//!
//! WHY: The proxy must survive restarts. Wraps `foundation_db::FileStateStore`
//! to persist backend drain/pause state, TLS data, and config hashes.
//!
//! WHAT: [`ProxyStateStore`] — load/save proxy state as JSON-serialized
//! `ResourceState` entries, one per domain keyed by "`proxy_state`".

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use foundation_core::valtron::sleep_async;

use foundation_db::core::errors::StorageError;
use foundation_db::core::state::types::ResourceState;
use foundation_db::core::state::{FileStateStore, StateStore};
use futures_util::lock::Mutex as AsyncMutex;
use serde::{Deserialize, Serialize};

use crate::config::ProxyConfig;

/// Persistent proxy state, serialized as JSON value inside `ResourceState.data`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedProxyState {
    pub backend_states: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert_pem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_pem: Option<String>,
    pub cert_renewed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_hash: Option<String>,
}

/// Default for [`ProxyStateStore::with_lock_timeout`]: 30 seconds. A
/// load-change-save of one small JSON file takes milliseconds, so a wait this
/// long means another process holds the lock without finishing (it is stuck).
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// Default for [`ProxyStateStore::with_lock_retry_interval`]: 5 milliseconds,
/// the wait between two tries to take a busy lock.
const DEFAULT_LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(5);

pub struct ProxyStateStore {
    store: FileStateStore,
    /// Serialises the load-change-save of the one `proxy_state` document inside
    /// this store object.
    ///
    /// WHY: `save_backend_state` and `save_tls_cert` run on separate
    /// control-socket threads. Each reads the whole state, changes one field and
    /// saves it back. Without a lock two commands can each save over the
    /// other's change (a lost update). The store keeps one document so old state
    /// files stay loadable; a lock is the smallest fix for that shape. It lets
    /// tasks of one store queue without polling the file lock.
    write_lock: AsyncMutex<()>,
    /// The lock file for the same read-modify-write across store objects and
    /// across processes (see [`Self::lock_state_file`]).
    lock_path: PathBuf,
    lock_timeout: Duration,
    lock_retry_interval: Duration,
}

/// An advisory lock on the state directory. The lock ends when this value is
/// dropped (the file closes), also when the process dies.
struct StateFileLock {
    _file: File,
}

const PROXY_STATE_ID: &str = "proxy_state";

/// Try to take a shared (`shared`) or exclusive `flock` on `file` without blocking. `Ok(false)`
/// means another holder has it.
fn try_flock(file: &File, shared: bool) -> std::io::Result<bool> {
    use std::os::fd::AsRawFd;
    loop {
        // SAFETY: `flock` reads no memory; the descriptor is open for the
        // lifetime of `file`.
        if unsafe {
            libc::flock(
                file.as_raw_fd(),
                (if shared { libc::LOCK_SH } else { libc::LOCK_EX }) | libc::LOCK_NB,
            )
        } == 0
        {
            return Ok(true);
        }
        let err = std::io::Error::last_os_error();
        match err.kind() {
            std::io::ErrorKind::WouldBlock => return Ok(false),
            std::io::ErrorKind::Interrupted => {}
            _ => return Err(err),
        }
    }
}

impl ProxyStateStore {
    #[must_use]
    pub fn new(state_dir: &Path, domain: &str) -> Self {
        let store = FileStateStore::new(state_dir, "foundation_proxy", domain);
        let lock_path = state_dir
            .join(".deployment")
            .join("foundation_proxy")
            .join(domain.replace('/', ":"))
            .join("proxy_state.lock");
        Self {
            store,
            write_lock: AsyncMutex::new(()),
            lock_path,
            lock_timeout: DEFAULT_LOCK_TIMEOUT,
            lock_retry_interval: DEFAULT_LOCK_RETRY_INTERVAL,
        }
    }

    /// Set how long a save waits for the state-file lock before it fails with
    /// `StorageError::Backend`. Default 30 seconds (a save of this file takes
    /// milliseconds, so a longer wait means a stuck holder).
    #[must_use]
    pub fn with_lock_timeout(mut self, timeout: Duration) -> Self {
        self.lock_timeout = timeout;
        self
    }

    /// Set the wait between two tries to take a busy state-file lock. Default
    /// 5 milliseconds. The wait is a timer wake, never a thread sleep.
    #[must_use]
    pub fn with_lock_retry_interval(mut self, interval: Duration) -> Self {
        self.lock_retry_interval = interval;
        self
    }

    /// Take the advisory lock that makes one load-change-save atomic across
    /// store objects and processes.
    ///
    /// WHY: the state store (`FileStateStore`) has no conditional write (no
    /// version or etag compare), so a second proxy process on one directory
    /// could load, change and save over a change made between its load and its
    /// save. `flock` on a file in the directory excludes every other holder, in
    /// this process or another, and the kernel drops it when the holder dies, so
    /// a crashed proxy cannot leave the directory locked.
    ///
    /// HOW: a try-lock loop that waits on the valtron timer between tries, so a
    /// busy lock parks the task and holds no thread.
    async fn lock_state_file(&self, shared: bool) -> Result<StateFileLock, StorageError> {
        if let Some(dir) = self.lock_path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| StorageError::Backend(format!("create {}: {e}", dir.display())))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.lock_path)
            .map_err(|e| {
                StorageError::Backend(format!("open {}: {e}", self.lock_path.display()))
            })?;
        let deadline = Instant::now() + self.lock_timeout;
        loop {
            match try_flock(&file, shared) {
                Ok(true) => return Ok(StateFileLock { _file: file }),
                Ok(false) => {
                    if Instant::now() >= deadline {
                        return Err(StorageError::Backend(format!(
                            "another proxy holds the state lock {} (waited {:?})",
                            self.lock_path.display(),
                            self.lock_timeout
                        )));
                    }
                    sleep_async(self.lock_retry_interval).await;
                }
                Err(e) => {
                    return Err(StorageError::Backend(format!(
                        "lock {}: {e}",
                        self.lock_path.display()
                    )));
                }
            }
        }
    }

    /// Load persisted proxy state, if any.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the state can't be read or decoded.
    pub async fn load(&self) -> Result<Option<PersistedProxyState>, StorageError> {
        // A shared lock excludes a writer (which holds the exclusive lock while
        // it writes the file in place), so a load never reads a torn file.
        let _file_lock = self.lock_state_file(true).await?;
        self.load_unlocked().await
    }

    /// Load without taking the state-file lock; the caller holds it.
    async fn load_unlocked(&self) -> Result<Option<PersistedProxyState>, StorageError> {
        match self.store.get(PROXY_STATE_ID).await? {
            Some(resource) => serde_json::from_value(resource.output)
                .map(Some)
                .map_err(|e| StorageError::Serialization(e.to_string())),
            None => Ok(None),
        }
    }

    /// Save current proxy state.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the state can't be encoded or written.
    pub async fn save(&self, state: &PersistedProxyState) -> Result<(), StorageError> {
        let data =
            serde_json::to_value(state).map_err(|e| StorageError::Serialization(e.to_string()))?;
        let resource = ResourceState {
            id: PROXY_STATE_ID.to_string(),
            kind: "proxy_state".to_string(),
            provider: "foundation_proxy".to_string(),
            status: foundation_db::core::state::types::StateStatus::Created,
            environment: None,
            config_hash: String::new(),
            output: data,
            config_snapshot: serde_json::Value::Null,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        self.store.set(PROXY_STATE_ID, &resource).await
    }

    /// Set a backend's state string.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the state can't be loaded or saved.
    pub async fn save_backend_state(
        &self,
        service: &str,
        backend_url: &str,
        state: &str,
    ) -> Result<(), StorageError> {
        let _guard = self.write_lock.lock().await;
        let _file_lock = self.lock_state_file(false).await?;
        let mut persisted = self.load_unlocked().await?.unwrap_or_default();
        persisted
            .backend_states
            .insert(format!("{service}/{backend_url}"), state.to_string());
        self.save(&persisted).await
    }

    /// Deterministic hash of proxy config for change detection.
    #[must_use]
    pub fn compute_config_hash(config: &ProxyConfig) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        config.domain.hash(&mut hasher);
        config.public_ip.hash(&mut hasher);
        for svc in &config.services {
            svc.name.hash(&mut hasher);
            svc.host.hash(&mut hasher);
            for be in &svc.backends {
                be.url.hash(&mut hasher);
            }
        }
        format!("{:016x}", hasher.finish())
    }

    /// Store TLS certificate data.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the state can't be loaded or saved.
    pub async fn save_tls_cert(&self, cert_pem: &str, key_pem: &str) -> Result<(), StorageError> {
        let _guard = self.write_lock.lock().await;
        let _file_lock = self.lock_state_file(false).await?;
        let mut state = self.load_unlocked().await?.unwrap_or_default();
        state.cert_pem = Some(cert_pem.to_string());
        state.key_pem = Some(key_pem.to_string());
        state.cert_renewed_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        self.save(&state).await
    }
}
