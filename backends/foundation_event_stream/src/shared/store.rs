//! Subscription state over the shared virtual file system (VFS), and the append observer.
//!
//! The store keeps the hot records in memory and writes a JSON snapshot to a control stream on
//! every change. A restarted host reloads the last snapshot of each subscription. The observer
//! wraps the host's VFS and tells the engine after an append, so the transport-neutral core never
//! depends on the Hypertext Transfer Protocol (HTTP) handler.

use crate::shared::config::SubscriptionConfig;
use crate::shared::model::SubscriptionRecord;
use foundation_context::{Clock, Ctx};
use foundation_core::durable::{
    Chunk, ContentType, CreateSpec, DurableStream, DurableStreamStore, DurableStreamVfs,
    NotifierRef, ReadFrom, ReadLimit, ReadRequest, StreamName, VfsEntry, VfsFuture, VfsMeta,
    VfsOffset,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A receiver of the append signal.
pub trait StreamWriteObserver: Send + Sync {
    /// Handle an append to `name`.
    fn stream_written(&self, name: StreamName) -> VfsFuture<'_, ()>;
}

/// A virtual file system (VFS) wrapper that reports every append to an observer.
pub struct ObservingVfs {
    inner: Arc<dyn DurableStreamVfs>,
    observer: Arc<dyn StreamWriteObserver>,
}

impl ObservingVfs {
    /// Wrap a virtual file system (VFS).
    #[must_use]
    pub fn new(
        inner: Arc<dyn DurableStreamVfs>,
        observer: Arc<dyn StreamWriteObserver>,
    ) -> Arc<Self> {
        Arc::new(Self { inner, observer })
    }
}

impl DurableStreamVfs for ObservingVfs {
    fn create(&self, name: &StreamName, meta: VfsMeta) -> VfsFuture<'_, bool> {
        self.inner.create(name, meta)
    }

    fn append(&self, name: &StreamName, bytes: &[u8]) -> VfsFuture<'_, VfsOffset> {
        let inner = self.inner.clone();
        let observer = self.observer.clone();
        let name = name.clone();
        let bytes = bytes.to_vec();
        Box::pin(async move {
            let offset = inner.append(&name, &bytes).await?;
            observer.stream_written(name.clone()).await?;
            Ok(offset)
        })
    }

    fn append_with_meta(
        &self,
        name: &StreamName,
        bytes: &[u8],
        meta: VfsMeta,
        expected_version: u64,
    ) -> VfsFuture<'_, VfsOffset> {
        let inner = self.inner.clone();
        let observer = self.observer.clone();
        let name = name.clone();
        let bytes = bytes.to_vec();
        Box::pin(async move {
            let offset = inner
                .append_with_meta(&name, &bytes, meta, expected_version)
                .await?;
            observer.stream_written(name.clone()).await?;
            Ok(offset)
        })
    }

    fn read_at(&self, name: &StreamName, at: &VfsOffset, limit: usize) -> VfsFuture<'_, Vec<u8>> {
        self.inner.read_at(name, at, limit)
    }

    fn meta(&self, name: &StreamName) -> VfsFuture<'_, VfsMeta> {
        self.inner.meta(name)
    }

    fn touch(&self, name: &StreamName, now_ms: i64) -> VfsFuture<'_, ()> {
        self.inner.touch(name, now_ms)
    }

    fn trim(&self, name: &StreamName, at: VfsOffset) -> VfsFuture<'_, ()> {
        self.inner.trim(name, at)
    }

    fn close(&self, name: &StreamName) -> VfsFuture<'_, ()> {
        self.inner.close(name)
    }

    fn remove(&self, name: &StreamName) -> VfsFuture<'_, bool> {
        self.inner.remove(name)
    }

    fn list(&self, prefix: &StreamName) -> VfsFuture<'_, Vec<VfsEntry>> {
        self.inner.list(prefix)
    }

    fn notifier(&self, name: &StreamName) -> Option<NotifierRef> {
        self.inner.notifier(name)
    }
}

/// The subscription state store.
pub struct SubscriptionStore {
    vfs: Arc<dyn DurableStreamVfs>,
    stream: DurableStream,
    state_prefix: StreamName,
    config: SubscriptionConfig,
    records: Mutex<BTreeMap<String, SubscriptionRecord>>,
    loaded: AtomicBool,
}

impl SubscriptionStore {
    /// Build a store over a virtual file system (VFS) and the clock of `ctx`.
    #[must_use]
    pub fn new(vfs: Arc<dyn DurableStreamVfs>, ctx: &Ctx, config: SubscriptionConfig) -> Self {
        let clock: Arc<dyn Clock> = ctx.clock();
        let state_prefix = StreamName::parse(&config.state_prefix())
            .unwrap_or_else(|_| StreamName::parse("__ds/state").expect("valid fallback prefix"));
        Self {
            stream: DurableStream::with_clock(vfs.clone(), clock),
            vfs,
            state_prefix,
            config,
            records: Mutex::new(BTreeMap::new()),
            loaded: AtomicBool::new(false),
        }
    }

    /// The stream store over the raw virtual file system (VFS).
    #[must_use]
    pub fn stream(&self) -> &DurableStream {
        &self.stream
    }

    /// The raw virtual file system (VFS) under the store.
    #[must_use]
    pub fn raw_vfs(&self) -> &Arc<dyn DurableStreamVfs> {
        &self.vfs
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &SubscriptionConfig {
        &self.config
    }

    /// True when the persisted state has been read.
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::Acquire)
    }

    /// Read every persisted subscription snapshot.
    ///
    /// The call is idempotent. A second call returns without a read.
    ///
    /// # Errors
    ///
    /// Returns the storage error of the list or read.
    pub async fn load(&self) -> Result<(), String> {
        if self.is_loaded() {
            return Ok(());
        }
        let entries = self
            .vfs
            .list(&self.state_prefix)
            .await
            .map_err(|error| error.to_string())?;
        for entry in entries {
            let page = match self
                .stream
                .read(
                    &entry.name,
                    ReadRequest {
                        from: ReadFrom::Start,
                        limit: ReadLimit::new(Some(usize::MAX), None),
                    },
                )
                .await
            {
                Ok(page) => page,
                Err(error) => {
                    tracing::warn!(
                        stream = entry.name.as_str(),
                        "subscription state read failed: {error}"
                    );
                    continue;
                }
            };
            let mut bytes = Vec::new();
            for chunk in page.chunks {
                bytes.extend_from_slice(chunk.as_bytes());
            }
            match parse_snapshot(&bytes) {
                Some(Some(record)) => {
                    self.records
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .insert(record.id.clone(), record);
                }
                Some(None) => {}
                None => tracing::warn!(
                    stream = entry.name.as_str(),
                    "subscription state snapshot is not readable"
                ),
            }
        }
        self.loaded.store(true, Ordering::Release);
        Ok(())
    }

    /// Run a closure with the records.
    pub fn with_records<R>(
        &self,
        action: impl FnOnce(&BTreeMap<String, SubscriptionRecord>) -> R,
    ) -> R {
        let records = self
            .records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        action(&records)
    }

    /// Run a closure with one record.
    pub fn with_record<R>(
        &self,
        id: &str,
        action: impl FnOnce(Option<&SubscriptionRecord>) -> R,
    ) -> R {
        let records = self
            .records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        action(records.get(id))
    }

    /// Run a closure with one record for mutation.
    pub fn with_record_mut<R>(
        &self,
        id: &str,
        action: impl FnOnce(Option<&mut SubscriptionRecord>) -> R,
    ) -> R {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        action(records.get_mut(id))
    }

    /// Return a copy of one record.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<SubscriptionRecord> {
        self.with_record(id, |record| record.cloned())
    }

    /// Return a copy of every record, sorted by identifier.
    #[must_use]
    pub fn all(&self) -> Vec<SubscriptionRecord> {
        self.with_records(|records| records.values().cloned().collect())
    }

    /// Store a record in memory.
    pub fn insert(&self, record: SubscriptionRecord) {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(record.id.clone(), record);
    }

    /// Store a record and write its snapshot.
    ///
    /// # Errors
    ///
    /// Returns the storage error of the snapshot write.
    pub async fn save(&self, record: &SubscriptionRecord) -> Result<(), String> {
        self.insert(record.clone());
        self.persist(record).await
    }

    /// Remove a record. The persisted snapshot stays as a tombstone for the state stream; the
    /// in-memory map no longer lists it.
    pub fn remove(&self, id: &str) -> Option<SubscriptionRecord> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id)
    }

    /// Append the record snapshot to its state stream.
    ///
    /// # Errors
    ///
    /// Returns the storage error of the create or append.
    pub async fn persist(&self, record: &SubscriptionRecord) -> Result<(), String> {
        let bytes = serde_json::to_vec(record).map_err(|error| error.to_string())?;
        self.append_state(&record.id, bytes).await
    }

    /// Append a deletion marker to the state stream of `id`.
    ///
    /// A restart reads the marker and does not restore the subscription.
    ///
    /// # Errors
    ///
    /// Returns the storage error of the create or append.
    pub async fn persist_tombstone(&self, id: &str) -> Result<(), String> {
        self.append_state(id, br#"{"deleted":true}"#.to_vec()).await
    }

    async fn append_state(&self, id: &str, mut bytes: Vec<u8>) -> Result<(), String> {
        let name = self.state_stream_name(id);
        let content_type =
            ContentType::parse("application/octet-stream").map_err(|error| error.to_string())?;
        self.stream
            .create(
                name.clone(),
                CreateSpec {
                    content_type: Some(content_type),
                    ..CreateSpec::default()
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        self.stream
            .append(
                &name,
                foundation_core::durable::AppendRequest::new(Chunk::new(bytes)),
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// List streams under a prefix.
    ///
    /// # Errors
    ///
    /// Returns the storage error of the list.
    pub async fn list(&self, prefix: &StreamName) -> Result<Vec<VfsEntry>, String> {
        self.vfs
            .list(prefix)
            .await
            .map_err(|error| error.to_string())
    }

    /// Read every byte of a stream.
    ///
    /// # Errors
    ///
    /// Returns the storage error of the read.
    pub async fn read_all(&self, name: &StreamName) -> Result<Vec<u8>, String> {
        let page = self
            .stream
            .read(
                name,
                ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::new(Some(usize::MAX), None),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        let mut bytes = Vec::new();
        for chunk in page.chunks {
            bytes.extend_from_slice(chunk.as_bytes());
        }
        Ok(bytes)
    }

    fn state_stream_name(&self, id: &str) -> StreamName {
        let digest = Sha256::digest(id.as_bytes());
        let mut suffix = String::with_capacity(digest.len() * 2);
        for byte in digest {
            suffix.push_str(&format!("{byte:02x}"));
        }
        StreamName::parse(&format!("{}/{}", self.state_prefix.as_str(), suffix))
            .unwrap_or_else(|_| self.state_prefix.clone())
    }
}

/// Parse the last snapshot line of a state stream.
///
/// Returns `Some(None)` for a deletion marker, `Some(Some(record))` for a snapshot, and `None`
/// when the bytes do not hold a readable snapshot.
fn parse_snapshot(bytes: &[u8]) -> Option<Option<SubscriptionRecord>> {
    let line = bytes
        .split(|byte| *byte == b'\n')
        .rev()
        .find(|line| !line.is_empty())?;
    let value: serde_json::Value = serde_json::from_slice(line).ok()?;
    if value.get("deleted").and_then(serde_json::Value::as_bool) == Some(true) {
        return Some(None);
    }
    serde_json::from_value(value).ok().map(Some)
}
