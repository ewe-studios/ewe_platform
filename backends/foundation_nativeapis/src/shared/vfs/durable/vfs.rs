//! The VFS core over one object store.
//!
//! The core owns the spool, the batcher, the head compare-and-swap (CAS), the
//! read path, the caches and the object lifecycle. The S3 and R2 wrappers in
//! `native/vfs/{s3,r2}.rs` add only provider settings.

use super::head::{
    content_hash, head_key, is_segment_key, lease_key, parse_head_key, parse_stream_key,
    probe_prefix, segment_key, stream_prefix, HeadManifest, SegmentEntry,
};
use super::object_store::{ObjectError, ObjectStore, Version};
use super::spool::Spool;
use super::{CasMode, ObjectLayout, ObjectVfsConfig, Provider, ProviderSettings, PublishPolicy};
use core::future::Future;
use core::pin::Pin;
use foundation_compact::time::unix_millis;
use foundation_context::Ctx;
use foundation_core::durable::shared::sql::block::{
    decode_index, encode_block, extract_range, payload_range, read_header, BLOCK_ENTRY_BYTES,
    BLOCK_HEADER_BYTES,
};
use foundation_core::durable::{
    DurableStreamVfs, Notifier, NotifierRef, StreamName, VfsEntry, VfsError, VfsFuture, VfsMeta,
    VfsOffset,
};
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};

/// One notifier for a stream.
#[derive(Default)]
pub struct ObjectNotifier {
    callbacks: Mutex<Vec<Box<dyn Fn() + Send + 'static>>>,
}

impl ObjectNotifier {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Box<dyn Fn() + Send + 'static>>> {
        self.callbacks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Notifier for ObjectNotifier {
    fn register(&self, callback: Box<dyn Fn() + Send + 'static>) {
        self.lock().push(callback);
    }

    fn notify(&self) {
        let callbacks = std::mem::take(&mut *self.lock());
        for callback in callbacks {
            callback();
        }
    }
}

/// What one flush did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlushReport {
    /// The number of immutable objects put.
    pub blocks_put: usize,
    /// The number of bytes put.
    pub bytes_put: u64,
    /// The number of streams that published a batch.
    pub streams_flushed: usize,
    /// The number of head CAS retries.
    pub retries: u32,
}

/// What one lifecycle pass did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LifecycleReport {
    /// The number of objects that a head references.
    pub live: usize,
    /// The number of unreferenced objects that still wait for the grace window.
    pub retired: usize,
    /// The number of objects whose delete was issued.
    pub delete_issued: usize,
    /// The number of objects now absent.
    pub deleted: usize,
    /// Every key that a delete refused, with the store's message.
    pub errors: Vec<(String, String)>,
}

/// What one compaction did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompactionReport {
    /// The number of segment pairs merged.
    pub merged: usize,
    /// The number of new objects written.
    pub objects_written: usize,
}

/// The result of the capability probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeReport {
    /// True when the store refused a stale `If-Match` write.
    pub conditional_write: bool,
    /// True when the store reports multipart support.
    pub multipart: bool,
    /// The largest single PUT.
    pub max_single_put_bytes: u64,
    /// True when the store reports CRC32 checksums.
    pub checksum_crc32: bool,
}

/// The provider capability record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCapabilities {
    /// The provider.
    pub provider: Provider,
    /// The store capabilities.
    pub store: super::object_store::ObjectCapabilities,
    /// The compare-and-swap mode in use.
    pub cas: CasMode,
    /// The object layout.
    pub layout: ObjectLayout,
    /// The explicit provider settings.
    pub settings: ProviderSettings,
}

#[derive(Clone, Debug)]
struct CachedHead {
    manifest: HeadManifest,
    version: Version,
    fetched_ms: i64,
}

type HeadLoadFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(HeadManifest, Version), VfsError>> + Send + 'a>>;
type CasFuture<'a> = Pin<Box<dyn Future<Output = Result<Version, VfsError>> + Send + 'a>>;
type AppendFuture<'a> = Pin<Box<dyn Future<Output = Result<VfsOffset, VfsError>> + Send + 'a>>;
type CombinedFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(VfsMeta, Version, HeadManifest), VfsError>> + Send + 'a>>;
type ReadChainFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, VfsError>> + Send + 'a>>;
type ReadOnceFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(Vec<u8>, Version), VfsError>> + Send + 'a>>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RangeKey {
    key: String,
    start: u64,
    len: usize,
}

#[derive(Default)]
struct LocalState {
    heads: HashMap<StreamName, CachedHead>,
    ranges: HashMap<RangeKey, Vec<u8>>,
    range_bytes: usize,
    probe: Option<ProbeReport>,
}

/// A durable stream VFS over one object store.
pub struct ObjectVfs {
    store: Arc<dyn ObjectStore>,
    ctx: Ctx,
    config: ObjectVfsConfig,
    spool: Spool,
    settings: ProviderSettings,
    state: Mutex<LocalState>,
    notifiers: Mutex<HashMap<StreamName, Arc<ObjectNotifier>>>,
}

impl fmt::Debug for ObjectVfs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectVfs")
            .field("config", &self.config)
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl ObjectVfs {
    /// Build a VFS over `store`.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Unsupported`] when the configuration names no spool
    /// root, or an input/output error when the spool cannot be created.
    pub fn new(
        store: Arc<dyn ObjectStore>,
        ctx: &Ctx,
        config: ObjectVfsConfig,
        settings: ProviderSettings,
    ) -> Result<Self, VfsError> {
        if config.spool_root.as_os_str().is_empty() {
            return Err(VfsError::Unsupported);
        }
        let spool = Spool::new(&config.spool_root).map_err(io_error)?;
        Ok(Self {
            store,
            ctx: ctx.clone(),
            config,
            spool,
            settings,
            state: Mutex::new(LocalState::default()),
            notifiers: Mutex::new(HashMap::new()),
        })
    }

    /// Return the configuration.
    #[must_use]
    pub fn config(&self) -> &ObjectVfsConfig {
        &self.config
    }

    /// Return the object store.
    #[must_use]
    pub fn store(&self) -> &Arc<dyn ObjectStore> {
        &self.store
    }

    /// Return the local spool.
    #[must_use]
    pub fn spool(&self) -> &Spool {
        &self.spool
    }

    /// Return the provider settings.
    #[must_use]
    pub fn settings(&self) -> &ProviderSettings {
        &self.settings
    }

    fn now_ms(&self) -> i64 {
        unix_millis(self.ctx.clock().wall_now())
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, LocalState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn notifier(&self, name: &StreamName) -> Arc<ObjectNotifier> {
        self.notifiers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(name.clone())
            .or_default()
            .clone()
    }

    fn forget_notifier(&self, name: &StreamName) {
        self.notifiers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name);
    }

    fn notify(&self, name: &StreamName) {
        self.notifier(name).notify();
    }

    /// Return the compare-and-swap mode from the capability record.
    #[must_use]
    pub fn cas_mode(&self) -> CasMode {
        let probed = self.lock_state().probe.map(|probe| probe.conditional_write);
        let conditional = probed.unwrap_or_else(|| self.store.capabilities().conditional_write);
        if conditional {
            CasMode::ConditionalPut
        } else {
            CasMode::Lease
        }
    }

    /// Return the provider capability record.
    #[must_use]
    pub fn provider_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider: self.settings.provider,
            store: self.store.capabilities(),
            cas: self.cas_mode(),
            layout: self.config.layout,
            settings: self.settings.clone(),
        }
    }

    /// Probe the store for conditional write support and checksums.
    ///
    /// The probe writes one small object, then checks that a stale `If-Match`
    /// update is refused. It deletes the probe object.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the probe cannot complete.
    pub async fn probe(&self) -> Result<ProbeReport, VfsError> {
        let prefix = probe_prefix(&self.config.root);
        let mut key = format!("{prefix}{}", self.now_ms());
        let mut attempt = 0_u32;
        let version = loop {
            match self.store.put_if_none_match(&key, b"probe".to_vec()).await {
                Ok(version) => break version,
                Err(ObjectError::PreconditionFailed) => {
                    attempt = attempt.saturating_add(1);
                    if attempt > self.config.retry_bound {
                        return Err(VfsError::Conflict);
                    }
                    key = format!("{prefix}{}-{attempt}", self.now_ms());
                }
                Err(error) => return Err(object_error(error)),
            }
        };
        let conditional_write = match self
            .store
            .put_if_match(&key, b"probe-1".to_vec(), &version)
            .await
        {
            Ok(_) => {
                // A stale If-Match must fail. A store that writes anyway has no
                // conditional write.
                self.store
                    .put_if_match(&key, b"probe-2".to_vec(), &version)
                    .await
                    .is_err()
            }
            Err(ObjectError::PreconditionFailed) => false,
            Err(error) => return Err(object_error(error)),
        };
        let _ = self.store.delete(&key).await;
        let capabilities = self.store.capabilities();
        let report = ProbeReport {
            conditional_write,
            multipart: capabilities.multipart,
            max_single_put_bytes: capabilities.max_single_put_bytes,
            checksum_crc32: capabilities.checksum_crc32,
        };
        self.lock_state().probe = Some(report);
        Ok(report)
    }

    fn cache_head(&self, name: &StreamName, manifest: HeadManifest, version: Version) {
        let fetched_ms = self.now_ms();
        self.lock_state().heads.insert(
            name.clone(),
            CachedHead {
                manifest,
                version,
                fetched_ms,
            },
        );
    }

    fn forget_head(&self, name: &StreamName) {
        self.lock_state().heads.remove(name);
    }

    fn load_head<'a>(
        &'a self,
        name: &'a StreamName,
        allow_soft: bool,
        fresh: bool,
    ) -> HeadLoadFuture<'a> {
        Box::pin(async move {
            let now = self.now_ms();
            if !fresh {
                let cached = self.lock_state().heads.get(name).cloned();
                if let Some(cached) = cached {
                    if now.saturating_sub(cached.fetched_ms) <= self.config.head_cache_ms {
                        if cached.manifest.soft_deleted && !allow_soft {
                            return Err(VfsError::Gone);
                        }
                        return Ok((cached.manifest, cached.version));
                    }
                }
            }
            let key = head_key(&self.config.root, name);
            let Some(meta) = self.store.head(&key).await.map_err(object_error)? else {
                self.forget_head(name);
                return Err(VfsError::NotFound);
            };
            let cached = self.lock_state().heads.get(name).cloned();
            if let Some(cached) = cached {
                if cached.version == meta.version {
                    if cached.manifest.soft_deleted && !allow_soft {
                        return Err(VfsError::Gone);
                    }
                    self.cache_head(name, cached.manifest.clone(), cached.version.clone());
                    return Ok((cached.manifest, cached.version));
                }
            }
            let bytes = self.store.get(&key).await.map_err(object_error)?;
            let text = String::from_utf8(bytes).map_err(|_| {
                VfsError::Io(Box::new(ObjectError::Unsupported(
                    "head manifest is not text",
                )))
            })?;
            let manifest = HeadManifest::parse(&text)?;
            if manifest.soft_deleted && !allow_soft {
                self.cache_head(name, manifest, meta.version);
                return Err(VfsError::Gone);
            }
            self.cache_head(name, manifest.clone(), meta.version.clone());
            Ok((manifest, meta.version))
        })
    }

    fn cas_head<'a>(
        &'a self,
        name: &'a StreamName,
        expected: Option<Version>,
        next: &'a HeadManifest,
    ) -> CasFuture<'a> {
        Box::pin(async move {
            let key = head_key(&self.config.root, name);
            let bytes = next.render().into_bytes();
            let result = match expected {
                None => self.store.put_if_none_match(&key, bytes).await,
                Some(version) => match self.cas_mode() {
                    CasMode::ConditionalPut => self.store.put_if_match(&key, bytes, &version).await,
                    CasMode::Lease => {
                        return self.lease_put(name, &key, &version, bytes, next).await;
                    }
                },
            };
            match result {
                Ok(version) => {
                    self.cache_head(name, next.clone(), version.clone());
                    Ok(version)
                }
                Err(ObjectError::PreconditionFailed) => {
                    self.forget_head(name);
                    Err(VfsError::Conflict)
                }
                Err(error) => Err(object_error(error)),
            }
        })
    }

    /// Write a head under a lease object when the store has no conditional
    /// write.
    ///
    /// The lease object serialises the read and the write. The guarantee is
    /// weaker than a conditional PUT: a writer that stops while it holds the
    /// lease blocks other writers until the lease times out.
    async fn lease_put(
        &self,
        name: &StreamName,
        head_key: &str,
        expected: &Version,
        bytes: Vec<u8>,
        next: &HeadManifest,
    ) -> Result<Version, VfsError> {
        let key = lease_key(&self.config.root, name);
        let mut attempt = 0_u32;
        loop {
            let owner = self.now_ms().to_string().into_bytes();
            match self.store.put_if_none_match(&key, owner).await {
                Ok(_) => {}
                Err(ObjectError::PreconditionFailed) => {
                    let stale = match self.store.get(&key).await {
                        Ok(body) => {
                            let held_since = String::from_utf8(body)
                                .ok()
                                .and_then(|text| text.parse::<i64>().ok())
                                .unwrap_or(0);
                            let timeout = i64::try_from(self.config.lease_timeout.as_millis())
                                .unwrap_or(i64::MAX);
                            self.now_ms().saturating_sub(held_since) >= timeout
                        }
                        Err(ObjectError::NotFound) => true,
                        Err(error) => return Err(object_error(error)),
                    };
                    if stale {
                        let _ = self.store.delete(&key).await;
                    }
                    attempt = attempt.saturating_add(1);
                    if attempt > self.config.retry_bound {
                        return Err(VfsError::Conflict);
                    }
                    continue;
                }
                Err(error) => return Err(object_error(error)),
            }

            // Under the lease, confirm that the head is still the version the
            // caller read. A change means another writer won; the caller
            // re-reads and retries.
            let current = self.store.head(head_key).await.map_err(object_error)?;
            let still_expected = current.is_some_and(|meta| &meta.version == expected);
            if !still_expected {
                let _ = self.store.delete(&key).await;
                self.forget_head(name);
                return Err(VfsError::Conflict);
            }
            let result = self.store.put(head_key, bytes).await;
            let _ = self.store.delete(&key).await;
            return match result {
                Ok(version) => {
                    self.cache_head(name, next.clone(), version.clone());
                    Ok(version)
                }
                Err(error) => Err(object_error(error)),
            };
        }
    }

    fn combined_meta<'a>(&'a self, name: &'a StreamName, allow_soft: bool) -> CombinedFuture<'a> {
        Box::pin(async move {
            let (head, version) = self.load_head(name, allow_soft, false).await?;
            let state = self.spool.load(name).map_err(io_error)?;
            let pending = self.spool.pending_len(name).map_err(io_error)?;
            let overlay = self.spool.overlay(name).map_err(io_error)?;
            let mut meta = head.to_meta();
            if let Some(overlay) = overlay {
                meta.content_type = overlay.content_type;
                meta.closed = overlay.closed;
                meta.expires = overlay.expires;
                meta.writer = overlay.writer;
            }
            if let Some(last_access) = state.last_access_ms {
                meta.last_access_ms = meta.last_access_ms.max(last_access);
            }
            meta.len = head.len.saturating_add(pending);
            meta.version = head.version.saturating_add(state.appends);
            Ok((meta, version, head))
        })
    }

    fn append_bytes<'a>(
        &'a self,
        name: &'a StreamName,
        bytes: Vec<u8>,
        overlay: Option<VfsMeta>,
        expected_version: Option<u64>,
    ) -> AppendFuture<'a> {
        Box::pin(async move {
            let (meta, _version, _head) = self.combined_meta(name, false).await?;
            if meta.closed {
                return Err(VfsError::Conflict);
            }
            if let Some(expected) = expected_version {
                if meta.version != expected {
                    return Err(VfsError::Conflict);
                }
            }
            let now = self.now_ms();
            self.spool
                .append(name, &bytes, now, overlay.as_ref())
                .map_err(io_error)?;
            let offset = meta
                .len
                .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
            self.notify(name);
            if self.config.publish_policy == PublishPolicy::Immediate {
                self.flush_stream(name).await?;
            }
            Ok(VfsOffset(offset))
        })
    }

    /// Flush one stream now, whatever its batch age.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a PUT or the head CAS still fails after the
    /// retry bound.
    pub async fn flush_stream(&self, name: &StreamName) -> Result<FlushReport, VfsError> {
        self.flush_stream_inner(name, true).await
    }

    /// Flush every eligible stream, with at most
    /// [`ObjectVfsConfig::parallel_puts`] streams in flight.
    ///
    /// A stream is eligible when its pending bytes reach the block cap or its
    /// oldest append reaches the batch age.
    ///
    /// # Errors
    ///
    /// Returns the first storage error after every started stream completes.
    pub async fn flush(&self) -> Result<FlushReport, VfsError> {
        let now = self.now_ms();
        let streams = self.spool.streams().map_err(io_error)?;
        let mut eligible = Vec::new();
        for name in streams {
            let state = self.spool.load(&name).map_err(io_error)?;
            let pending = self.spool.pending_len(&name).map_err(io_error)?;
            if self.eligible(&state, pending, now) {
                eligible.push(name);
            }
        }
        let mut report = FlushReport::default();
        let mut first_error = None;
        let mut futures = FuturesUnordered::new();
        let mut iter = eligible.into_iter();
        loop {
            while futures.len() < self.config.parallel_puts {
                let Some(name) = iter.next() else { break };
                futures.push(async move {
                    let result = self.flush_stream_inner(&name, false).await;
                    (name, result)
                });
            }
            let Some(result) = futures.next().await else {
                break;
            };
            match result {
                (_, Ok(one)) => merge_flush(&mut report, &one),
                (_, Err(error)) => {
                    first_error = Some(error);
                }
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(report),
        }
    }

    fn eligible(&self, state: &super::spool::SpoolState, pending: u64, now_ms: i64) -> bool {
        if pending == 0 && state.appends == 0 {
            return false;
        }
        if pending >= u64::try_from(self.config.max_block_bytes).unwrap_or(u64::MAX) {
            return true;
        }
        let age = i64::try_from(self.config.batch_age.as_millis()).unwrap_or(i64::MAX);
        now_ms.saturating_sub(state.first_appended_ms) >= age
    }

    #[allow(clippy::too_many_lines)]
    async fn flush_stream_inner(
        &self,
        name: &StreamName,
        force: bool,
    ) -> Result<FlushReport, VfsError> {
        let mut report = FlushReport::default();
        let state = self.spool.load(name).map_err(io_error)?;
        let pending = self.spool.pending_len(name).map_err(io_error)?;
        if pending == 0 && state.appends == 0 {
            return Ok(report);
        }
        if !force && !self.eligible(&state, pending, self.now_ms()) {
            return Ok(report);
        }
        let (head, _etag) = match self.load_head(name, true, true).await {
            Ok(head) => head,
            // A deleted stream has no head. Its spool waits for the lifecycle
            // pass, because a fork may still read the data.
            Err(VfsError::NotFound) => return Ok(report),
            Err(error) => return Err(error),
        };
        if head.soft_deleted {
            return Ok(report);
        }

        // Recover a batch whose PUT landed but whose head CAS may not have.
        let inflight = self.spool.inflight(name).map_err(io_error)?;
        let mut blocks: Vec<(u64, String, Vec<u8>)> = Vec::new();
        let mut appends = state.appends;
        if let Some(batch) = inflight {
            let keys: Vec<String> = batch.blocks.iter().map(|(_, key)| key.clone()).collect();
            if head_has_keys(&head, &keys) {
                // The head already holds the batch. Drop its spool bytes.
                self.spool
                    .consume(name, batch.snapshot_len)
                    .map_err(io_error)?;
                self.spool
                    .settle(name, batch.appends, false)
                    .map_err(io_error)?;
                self.spool.clear_inflight(name).map_err(io_error)?;
                let current = self.spool.load(name).map_err(io_error)?;
                let remaining = self.spool.pending_len(name).map_err(io_error)?;
                if remaining == 0 && current.appends == 0 {
                    return Ok(report);
                }
                return Box::pin(self.flush_stream_inner(name, force)).await;
            }
            // The PUT did not reach the head. The keys are deterministic, so a
            // second PUT is safe. Rebuild the block bytes from the snapshot.
            let snapshot_len = batch.snapshot_len.min(pending);
            let snapshot = self
                .spool
                .read_range(name, 0, usize::try_from(snapshot_len).unwrap_or(usize::MAX))
                .map_err(io_error)?;
            let mut offset = 0_usize;
            let mut rebuilt = Vec::with_capacity(batch.blocks.len());
            for (len, key) in &batch.blocks {
                let end = offset.saturating_add(usize::try_from(*len).unwrap_or(usize::MAX));
                if end > snapshot.len() {
                    break;
                }
                rebuilt.push((*len, key.clone(), snapshot[offset..end].to_vec()));
                offset = end;
            }
            if rebuilt.len() == batch.blocks.len() && !rebuilt.is_empty() {
                blocks = rebuilt;
                appends = batch.appends;
            }
        }
        if blocks.is_empty() {
            let bytes = self
                .spool
                .read_range(name, 0, usize::try_from(pending).unwrap_or(usize::MAX))
                .map_err(io_error)?;
            if bytes.is_empty() {
                // A metadata-only batch: publish the overlay with no object.
                if state.appends == 0 {
                    return Ok(report);
                }
                blocks = Vec::new();
            } else {
                blocks = self.split_blocks(name, &bytes);
            }
        }

        let snapshot_len = blocks
            .iter()
            .fold(0_u64, |total, (len, _, _)| total.saturating_add(*len));
        let block_records: Vec<(u64, String)> = blocks
            .iter()
            .map(|(len, key, _)| (*len, key.clone()))
            .collect();
        self.spool
            .set_inflight(name, snapshot_len, appends, &block_records)
            .map_err(io_error)?;

        let mut total_bytes = 0_u64;
        for (len, key, bytes) in &blocks {
            let object = match self.config.layout {
                ObjectLayout::Segmented => {
                    encode_block(&[(0, bytes.as_slice())]).map_err(block_error)?
                }
                ObjectLayout::AppendOnly => bytes.clone(),
            };
            self.put_block(key, object).await?;
            total_bytes = total_bytes.saturating_add(*len);
            report.blocks_put = report.blocks_put.saturating_add(1);
        }

        let overlay = self.spool.overlay(name).map_err(io_error)?;
        let mut attempt = 0_u32;
        loop {
            let (head, etag) = self.load_head(name, true, true).await?;
            let mut next = head.clone();
            let mut at = head.len;
            for (len, key, bytes) in &blocks {
                let batch = content_hash(bytes);
                next.segments.push(SegmentEntry {
                    at,
                    len: *len,
                    key: key.clone(),
                    batch,
                });
                at = at.saturating_add(*len);
            }
            next.len = head.len.saturating_add(total_bytes);
            next.published_own = head.published_own.saturating_add(total_bytes);
            next.version = head.version.saturating_add(appends);
            if let Some(overlay) = &overlay {
                next.apply_meta(overlay);
            }
            match self.cas_head(name, Some(etag), &next).await {
                Ok(_) => break,
                Err(VfsError::Conflict) => {
                    attempt = attempt.saturating_add(1);
                    report.retries = report.retries.saturating_add(1);
                    if attempt > self.config.retry_bound {
                        return Err(VfsError::Conflict);
                    }
                }
                Err(error) => return Err(error),
            }
        }
        self.spool.consume(name, snapshot_len).map_err(io_error)?;
        let after = self.spool.load(name).map_err(io_error)?;
        let clear_overlay = after.appends == appends;
        self.spool
            .settle(name, appends, clear_overlay)
            .map_err(io_error)?;
        self.spool.clear_inflight(name).map_err(io_error)?;
        report.bytes_put = report.bytes_put.saturating_add(total_bytes);
        report.streams_flushed = 1;
        self.notify(name);
        Ok(report)
    }

    fn split_blocks(&self, name: &StreamName, bytes: &[u8]) -> Vec<(u64, String, Vec<u8>)> {
        let target = self
            .config
            .segment_target_bytes
            .min(self.config.max_block_bytes)
            .max(1);
        let mut blocks = Vec::new();
        let mut offset = 0_usize;
        while offset < bytes.len() {
            let end = offset.saturating_add(target).min(bytes.len());
            let block = &bytes[offset..end];
            let batch = content_hash(block);
            let first = u64::try_from(offset).unwrap_or(u64::MAX);
            let last = u64::try_from(end).unwrap_or(u64::MAX);
            let key = segment_key(
                &self.config.root,
                name,
                self.config.layout,
                first,
                last,
                &batch,
            );
            blocks.push((last - first, key, block.to_vec()));
            offset = end;
        }
        blocks
    }

    async fn put_block(&self, key: &str, object: Vec<u8>) -> Result<(), VfsError> {
        let capabilities = self.store.capabilities();
        let mut attempt = 0_u32;
        loop {
            let result = if object.len() as u64 > self.config.max_single_put_bytes
                && capabilities.multipart
            {
                let part = self.config.multipart_part_bytes.max(1);
                let parts = object
                    .chunks(part)
                    .map(<[u8]>::to_vec)
                    .collect::<Vec<Vec<u8>>>();
                self.store.put_parts(key, parts).await
            } else {
                match self.store.put_if_none_match(key, object.clone()).await {
                    // The key is immutable and content addressed. An existing
                    // object with the same key holds the same bytes.
                    Err(ObjectError::PreconditionFailed) => return Ok(()),
                    other => other,
                }
            };
            match result {
                Ok(_) => return Ok(()),
                Err(error) => {
                    attempt = attempt.saturating_add(1);
                    if attempt > self.config.retry_bound {
                        return Err(object_error(error));
                    }
                }
            }
        }
    }

    fn read_chain(
        &self,
        name: StreamName,
        at: u64,
        limit: usize,
        depth: u32,
        direct: bool,
    ) -> ReadChainFuture<'_> {
        Box::pin(async move {
            if depth > self.config.max_fork_depth {
                return Err(VfsError::Conflict);
            }
            let (head, _version) = self.load_head(&name, !direct, false).await?;
            if direct && head.soft_deleted {
                return Err(VfsError::Gone);
            }
            if at < head.trim_from {
                return Err(VfsError::Trimmed);
            }
            if limit == 0 {
                return Ok(Vec::new());
            }
            let base = head.base();
            let mut out = Vec::new();
            if at < base {
                let take = usize::try_from(base - at).unwrap_or(usize::MAX).min(limit);
                if let Some(fork) = &head.fork {
                    out.extend(
                        self.read_chain(fork.source.clone(), at, take, depth + 1, false)
                            .await?,
                    );
                }
            }
            if out.len() < limit {
                let next = at.saturating_add(u64::try_from(out.len()).unwrap_or(u64::MAX));
                let room = limit - out.len();
                let pending = self.spool.pending_len(&name).map_err(io_error)?;
                let own_end = base.saturating_add(head.own_len()).saturating_add(pending);
                let end = next
                    .saturating_add(u64::try_from(room).unwrap_or(u64::MAX))
                    .min(own_end);
                if next < end {
                    out.extend(self.read_own(&name, &head, next, end, pending).await?);
                }
            }
            Ok(out)
        })
    }

    async fn read_own(
        &self,
        name: &StreamName,
        head: &HeadManifest,
        start: u64,
        end: u64,
        pending: u64,
    ) -> Result<Vec<u8>, VfsError> {
        if start >= end {
            return Ok(Vec::new());
        }
        let published_end = head.published_own;
        let mut out = Vec::new();
        let mut cursor = start;
        while cursor < end.min(published_end) {
            let Some(segment) = head.segments.iter().find(|segment| segment.covers(cursor)) else {
                return Err(missing_object(format!(
                    "no segment covers stream offset {cursor}"
                )));
            };
            let local_start = cursor.saturating_sub(segment.at);
            let stop = end.min(segment.end());
            let local_end = stop.saturating_sub(segment.at);
            let bytes = self.read_segment(segment, local_start, local_end).await?;
            let want = usize::try_from(local_end.saturating_sub(local_start)).unwrap_or(usize::MAX);
            if bytes.len() != want {
                return Err(missing_object(format!(
                    "segment {} returned {} of {want} bytes",
                    segment.key,
                    bytes.len()
                )));
            }
            out.extend_from_slice(&bytes);
            cursor = stop;
        }
        if cursor < end {
            let pending_own = cursor.saturating_sub(published_end);
            if pending_own >= pending {
                return Err(missing_object(format!(
                    "no data covers stream offset {cursor}"
                )));
            }
            let take = end
                .saturating_sub(cursor)
                .min(pending.saturating_sub(pending_own));
            let bytes = self
                .spool
                .read_range(
                    name,
                    pending_own,
                    usize::try_from(take).unwrap_or(usize::MAX),
                )
                .map_err(io_error)?;
            let want = usize::try_from(take).unwrap_or(usize::MAX);
            if bytes.len() != want {
                return Err(missing_object(format!(
                    "spool returned {} of {want} bytes",
                    bytes.len()
                )));
            }
            out.extend_from_slice(&bytes);
        }
        Ok(out)
    }

    async fn segment_range(&self, key: &str, start: u64, len: usize) -> Result<Vec<u8>, VfsError> {
        self.store
            .get_range(key, start, len)
            .await
            .map_err(|error| match error {
                // A live head names a segment. A missing segment is an error,
                // never zero bytes.
                ObjectError::NotFound => missing_object(format!("segment object {key} is missing")),
                other => object_error(other),
            })
    }

    async fn read_segment(
        &self,
        segment: &SegmentEntry,
        start: u64,
        end: u64,
    ) -> Result<Vec<u8>, VfsError> {
        if start >= end {
            return Ok(Vec::new());
        }
        let len = usize::try_from(end - start).unwrap_or(usize::MAX);
        let cache_key = RangeKey {
            key: segment.key.clone(),
            start,
            len,
        };
        if let Some(cached) = self.lock_state().ranges.get(&cache_key) {
            return Ok(cached.clone());
        }
        let bytes = match self.config.layout {
            ObjectLayout::Segmented => self.read_block(&segment.key, start, end).await?,
            ObjectLayout::AppendOnly => {
                let bytes = self.segment_range(&segment.key, start, len).await?;
                if bytes.len() != len {
                    return Err(missing_object(format!(
                        "segment {} returned {} of {len} bytes",
                        segment.key,
                        bytes.len()
                    )));
                }
                bytes
            }
        };
        self.cache_range(cache_key, bytes.clone());
        Ok(bytes)
    }

    async fn read_block(&self, key: &str, start: u64, end: u64) -> Result<Vec<u8>, VfsError> {
        let header_len = usize::try_from(BLOCK_HEADER_BYTES).unwrap_or(usize::MAX);
        let header = self.segment_range(key, 0, header_len).await?;
        let count = read_header(&header).map_err(block_error)?;
        let index_len = u64::from(count).saturating_mul(BLOCK_ENTRY_BYTES);
        let index = if index_len == 0 {
            Vec::new()
        } else {
            self.segment_range(
                key,
                BLOCK_HEADER_BYTES,
                usize::try_from(index_len).unwrap_or(usize::MAX),
            )
            .await?
        };
        let entries = decode_index(&index, count).map_err(block_error)?;
        let Some(range) = payload_range(&entries, start, end) else {
            return Err(missing_object(format!(
                "block {key} does not cover the read"
            )));
        };
        if range.len == 0 {
            return Ok(Vec::new());
        }
        let payload_start = BLOCK_HEADER_BYTES
            .saturating_add(index_len)
            .saturating_add(range.start);
        let payload = self
            .segment_range(
                key,
                payload_start,
                usize::try_from(range.len).unwrap_or(usize::MAX),
            )
            .await?;
        if payload.len() != usize::try_from(range.len).unwrap_or(usize::MAX) {
            return Err(missing_object(format!(
                "block {key} returned a short payload"
            )));
        }
        Ok(extract_range(&entries, &payload, range.start, start, end))
    }

    fn cache_range(&self, key: RangeKey, bytes: Vec<u8>) {
        let mut state = self.lock_state();
        state.range_bytes = state.range_bytes.saturating_add(bytes.len());
        state.ranges.insert(key, bytes);
        if state.range_bytes > self.config.range_cache_bytes {
            state.ranges.clear();
            state.range_bytes = 0;
        }
    }

    fn read_once<'a>(&'a self, name: &'a StreamName, at: u64, limit: usize) -> ReadOnceFuture<'a> {
        Box::pin(async move {
            let (head, version) = self.load_head(name, false, false).await?;
            if at < head.trim_from {
                return Err(VfsError::Trimmed);
            }
            let bytes = self.read_chain(name.clone(), at, limit, 0, true).await?;
            Ok((bytes, version))
        })
    }

    /// Remove every unreferenced object whose grace window ended.
    ///
    /// With `only = None` the pass covers every stream whose head exists. It
    /// never deletes an object that a head references, including a
    /// soft-deleted head whose bytes a fork still reads.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a LIST or a delete fails.
    pub async fn gc(&self, only: Option<&StreamName>) -> Result<LifecycleReport, VfsError> {
        let names = match only {
            Some(name) => vec![name.clone()],
            None => self.all_stream_names().await?,
        };
        let now = self.now_ms();
        let grace = i64::try_from(self.config.lifecycle_grace.as_millis()).unwrap_or(i64::MAX);
        let mut report = LifecycleReport::default();
        for name in names {
            let (head, etag) = match self.load_head(&name, true, true).await {
                Ok(head) => (Some(head.0), Some(head.1)),
                Err(VfsError::NotFound) => (None, None),
                Err(error) => return Err(error),
            };
            let head = match head {
                Some(head) => {
                    if head.trim_from > 0 {
                        self.drop_trimmed(&name, etag.as_ref(), &head).await?
                    } else {
                        head
                    }
                }
                None => HeadManifest::default(),
            };
            let referenced: HashSet<&str> = head
                .segments
                .iter()
                .map(|segment| segment.key.as_str())
                .collect();
            let prefix = stream_prefix(&self.config.root, &name);
            let objects = self.store.list(&prefix).await.map_err(object_error)?;
            let mut to_delete = Vec::new();
            for object in objects {
                if !is_segment_key(&object.key) {
                    continue;
                }
                if referenced.contains(object.key.as_str()) {
                    report.live = report.live.saturating_add(1);
                    continue;
                }
                if now.saturating_sub(object.last_modified_ms) < grace {
                    report.retired = report.retired.saturating_add(1);
                    continue;
                }
                report.delete_issued = report.delete_issued.saturating_add(1);
                to_delete.push(object.key);
            }
            if !to_delete.is_empty() {
                let deleted = self
                    .store
                    .delete_many(to_delete)
                    .await
                    .map_err(object_error)?;
                report.deleted = report.deleted.saturating_add(deleted.deleted.len());
                report.errors.extend(deleted.errors);
            }
        }
        if report.errors.is_empty() {
            Ok(report)
        } else {
            Err(VfsError::Io(Box::new(DeleteFailure(report.errors))))
        }
    }

    async fn drop_trimmed(
        &self,
        name: &StreamName,
        etag: Option<&Version>,
        head: &HeadManifest,
    ) -> Result<HeadManifest, VfsError> {
        let Some(etag) = etag else {
            return Ok(head.clone());
        };
        let mut next = head.clone();
        next.segments
            .retain(|segment| segment.end() > head.trim_from);
        if next.segments.len() == head.segments.len() {
            return Ok(next);
        }
        next.version = head.version.saturating_add(1);
        self.cas_head(name, Some(etag.clone()), &next).await?;
        Ok(next)
    }

    async fn all_stream_names(&self) -> Result<Vec<StreamName>, VfsError> {
        let prefix = format!("{}/", self.config.root.trim_end_matches('/'));
        let objects = self.store.list(&prefix).await.map_err(object_error)?;
        let mut names = Vec::new();
        for object in objects {
            if let Some(name) = parse_stream_key(&self.config.root, &object.key) {
                names.push(name);
            }
        }
        names.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        names.dedup();
        Ok(names)
    }

    async fn list_stream_names(&self) -> Result<Vec<StreamName>, VfsError> {
        let prefix = format!("{}/", self.config.root.trim_end_matches('/'));
        let objects = self.store.list(&prefix).await.map_err(object_error)?;
        let mut names = Vec::new();
        for object in objects {
            if let Some(name) = parse_head_key(&self.config.root, &object.key) {
                names.push(name);
            }
        }
        names.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        names.dedup();
        Ok(names)
    }

    /// Merge small adjacent segments into one object.
    ///
    /// The merge never copies a whole stream. It joins a pair whose combined
    /// length stays within the segment target.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a read, a PUT or the head CAS fails.
    pub async fn compact(&self, name: &StreamName) -> Result<CompactionReport, VfsError> {
        let (head, etag) = self.load_head(name, false, true).await?;
        if self.config.layout == ObjectLayout::AppendOnly {
            // The append-only layout never rewrites a log object.
            return Ok(CompactionReport::default());
        }
        let target = u64::try_from(self.config.segment_target_bytes).unwrap_or(u64::MAX);
        let mut segments = head.segments.clone();
        let mut merged = 0_usize;
        let mut written = 0_usize;
        let mut index = 0_usize;
        while index + 1 < segments.len() {
            let left = segments[index].clone();
            let right = segments[index + 1].clone();
            let combined = left.len.saturating_add(right.len);
            if combined > target {
                index += 1;
                continue;
            }
            let mut bytes = self.read_segment(&left, 0, left.len).await?;
            bytes.extend(self.read_segment(&right, 0, right.len).await?);
            let batch = content_hash(&bytes);
            let key = segment_key(
                &self.config.root,
                name,
                ObjectLayout::Segmented,
                left.at,
                right.end(),
                &batch,
            );
            let object = encode_block(&[(0, bytes.as_slice())]).map_err(block_error)?;
            self.put_block(&key, object).await?;
            written = written.saturating_add(1);
            segments[index] = SegmentEntry {
                at: left.at,
                len: combined,
                key,
                batch,
            };
            segments.remove(index + 1);
            merged = merged.saturating_add(1);
        }
        if merged == 0 {
            return Ok(CompactionReport::default());
        }
        let mut next = head;
        next.segments = segments;
        next.version = next.version.saturating_add(1);
        self.cas_head(name, Some(etag), &next).await?;
        Ok(CompactionReport {
            merged,
            objects_written: written,
        })
    }

    /// Return the lifecycle state of one object key.
    ///
    /// The method reads the head and the object listing. It does not change
    /// anything.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a HEAD or a LIST fails.
    pub async fn lifecycle(
        &self,
        name: &StreamName,
        key: &str,
    ) -> Result<super::ObjectLifecycle, VfsError> {
        let head = match self.load_head(name, true, true).await {
            Ok((head, _)) => head,
            Err(VfsError::NotFound) => HeadManifest::default(),
            Err(error) => return Err(error),
        };
        let live = head.segments.iter().any(|segment| segment.key == key);
        if live {
            return Ok(super::ObjectLifecycle::Live);
        }
        let Some(meta) = self.store.head(key).await.map_err(object_error)? else {
            return Ok(super::ObjectLifecycle::Deleted);
        };
        let grace = i64::try_from(self.config.lifecycle_grace.as_millis()).unwrap_or(i64::MAX);
        if self.now_ms().saturating_sub(meta.last_modified_ms) < grace {
            Ok(super::ObjectLifecycle::Retired)
        } else {
            Ok(super::ObjectLifecycle::DeleteIssued)
        }
    }
}

fn head_has_keys(head: &HeadManifest, keys: &[String]) -> bool {
    !keys.is_empty()
        && keys
            .iter()
            .all(|key| head.segments.iter().any(|segment| &segment.key == key))
}

fn merge_flush(target: &mut FlushReport, one: &FlushReport) {
    target.blocks_put = target.blocks_put.saturating_add(one.blocks_put);
    target.bytes_put = target.bytes_put.saturating_add(one.bytes_put);
    target.streams_flushed = target.streams_flushed.saturating_add(one.streams_flushed);
    target.retries = target.retries.saturating_add(one.retries);
}

fn object_error(error: ObjectError) -> VfsError {
    match error {
        ObjectError::NotFound => VfsError::NotFound,
        ObjectError::PreconditionFailed => VfsError::Conflict,
        other => VfsError::Io(Box::new(other)),
    }
}

fn block_error(error: foundation_core::durable::shared::sql::block::BlockError) -> VfsError {
    VfsError::Io(Box::new(error))
}

fn io_error(error: std::io::Error) -> VfsError {
    VfsError::Io(Box::new(error))
}

#[derive(Debug)]
struct MissingObject(String);

impl fmt::Display for MissingObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a live head has a missing object: {}", self.0)
    }
}

impl core::error::Error for MissingObject {}

fn missing_object(message: String) -> VfsError {
    VfsError::Io(Box::new(MissingObject(message)))
}

#[derive(Debug)]
struct DeleteFailure(Vec<(String, String)>);

impl fmt::Display for DeleteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} object deletes failed: {:?}", self.0.len(), self.0)
    }
}

impl core::error::Error for DeleteFailure {}

impl DurableStreamVfs for ObjectVfs {
    #[tracing::instrument(
        level = "info",
        skip_all,
        fields(
            stream = %name.as_str(),
            closed = meta.closed,
            fork = meta.fork.is_some(),
        )
    )]
    fn create(&self, name: &StreamName, meta: VfsMeta) -> VfsFuture<'_, bool> {
        let name = name.clone();
        Box::pin(async move {
            let key = head_key(&self.config.root, &name);
            match self.load_head(&name, true, true).await {
                Ok((existing, _)) => {
                    if existing.soft_deleted {
                        return Err(VfsError::Gone);
                    }
                    let same = existing.content_type.as_ref() == Some(&meta.content_type)
                        && existing.closed == meta.closed
                        && existing.expires == meta.expires
                        && existing.fork.as_ref().map(|fork| (&fork.source, fork.at))
                            == meta.fork.as_ref().map(|fork| (&fork.source, fork.at));
                    if same {
                        return Ok(false);
                    }
                    return Err(VfsError::Conflict);
                }
                Err(VfsError::NotFound) => {}
                Err(error) => return Err(error),
            }
            if let Some(fork) = &meta.fork {
                match self.combined_meta(&fork.source, true).await {
                    Ok((source, _, source_head)) => {
                        if source_head.soft_deleted {
                            return Err(VfsError::Gone);
                        }
                        if meta.len > source.len {
                            return Err(VfsError::Conflict);
                        }
                    }
                    Err(VfsError::NotFound) => return Err(VfsError::NotFound),
                    Err(error) => return Err(error),
                }
            }
            let mut manifest = HeadManifest::from_meta(&meta);
            manifest.version = 0;
            self.cas_head(&name, None, &manifest).await?;
            if let Some(fork) = &meta.fork {
                if let Err(error) = self.increment_refs(&fork.source, 1).await {
                    let _ = self.store.delete(&key).await;
                    self.forget_head(&name);
                    return Err(error);
                }
            }
            self.notify(&name);
            Ok(true)
        })
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %name.as_str(), bytes = bytes.len())
    )]
    fn append(&self, name: &StreamName, bytes: &[u8]) -> VfsFuture<'_, VfsOffset> {
        let name = name.clone();
        let bytes = bytes.to_vec();
        Box::pin(async move { self.append_bytes(&name, bytes, None, None).await })
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(
            stream = %name.as_str(),
            bytes = bytes.len(),
            expected_version = expected_version,
        )
    )]
    fn append_with_meta(
        &self,
        name: &StreamName,
        bytes: &[u8],
        meta: VfsMeta,
        expected_version: u64,
    ) -> VfsFuture<'_, VfsOffset> {
        let name = name.clone();
        let bytes = bytes.to_vec();
        Box::pin(async move {
            self.append_bytes(&name, bytes, Some(meta), Some(expected_version))
                .await
        })
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %name.as_str(), offset = ?at, limit_bytes = limit)
    )]
    fn read_at(&self, name: &StreamName, at: &VfsOffset, limit: usize) -> VfsFuture<'_, Vec<u8>> {
        let name = name.clone();
        let at = at.0;
        Box::pin(async move {
            let mut attempts = 0_u32;
            loop {
                let (bytes, version) = self.read_once(&name, at, limit).await?;
                let key = head_key(&self.config.root, &name);
                let current = self.store.head(&key).await.map_err(object_error)?;
                match current {
                    Some(meta) if meta.version == version => return Ok(bytes),
                    Some(_) => {
                        // The head changed during the read. Refresh it and
                        // repeat, so the bytes match one published state. The
                        // bound keeps a busy writer from stalling the reader.
                        self.forget_head(&name);
                        attempts = attempts.saturating_add(1);
                        if attempts > self.config.revalidation_retries {
                            return Ok(bytes);
                        }
                    }
                    None => {
                        self.forget_head(&name);
                        return Err(VfsError::NotFound);
                    }
                }
            }
        })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(stream = %name.as_str()))]
    fn meta(&self, name: &StreamName) -> VfsFuture<'_, VfsMeta> {
        let name = name.clone();
        Box::pin(async move {
            let (meta, _version, _head) = self.combined_meta(&name, false).await?;
            Ok(meta)
        })
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %name.as_str(), now_ms = now_ms)
    )]
    fn touch(&self, name: &StreamName, now_ms: i64) -> VfsFuture<'_, ()> {
        let name = name.clone();
        Box::pin(async move {
            let state = self.spool.load(&name).map_err(io_error)?;
            let pending = self.spool.pending_len(&name).map_err(io_error)?;
            if pending > 0 || state.appends > 0 {
                self.spool.touch(&name, now_ms).map_err(io_error)?;
                return Ok(());
            }
            let (head, etag) = self.load_head(&name, false, false).await?;
            let mut next = head.clone();
            next.last_access_ms = now_ms;
            self.cas_head(&name, Some(etag), &next).await?;
            Ok(())
        })
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %name.as_str(), at = ?at)
    )]
    fn trim(&self, name: &StreamName, at: VfsOffset) -> VfsFuture<'_, ()> {
        let name = name.clone();
        Box::pin(async move {
            let (meta, _version, head) = self.combined_meta(&name, false).await?;
            let (_, etag) = self.load_head(&name, false, true).await?;
            let mut next = head.clone();
            next.trim_from = at.0.min(meta.len).max(head.trim_from);
            next.version = head.version.saturating_add(1);
            self.cas_head(&name, Some(etag), &next).await?;
            Ok(())
        })
    }

    #[tracing::instrument(level = "info", skip_all, fields(stream = %name.as_str()))]
    fn close(&self, name: &StreamName) -> VfsFuture<'_, ()> {
        let name = name.clone();
        Box::pin(async move {
            let (mut meta, _version, _head) = self.combined_meta(&name, false).await?;
            meta.closed = true;
            let now = self.now_ms();
            self.spool
                .append(&name, &[], now, Some(&meta))
                .map_err(io_error)?;
            self.notify(&name);
            if self.config.publish_policy == PublishPolicy::Immediate {
                self.flush_stream(&name).await?;
            }
            Ok(())
        })
    }

    #[tracing::instrument(level = "info", skip_all, fields(stream = %name.as_str()))]
    fn remove(&self, name: &StreamName) -> VfsFuture<'_, bool> {
        let name = name.clone();
        Box::pin(async move {
            let (head, etag) = self.load_head(&name, false, true).await?;
            if head.ref_count > 0 {
                let mut next = head.clone();
                next.soft_deleted = true;
                next.version = head.version.saturating_add(1);
                self.cas_head(&name, Some(etag), &next).await?;
                self.notify(&name);
                return Ok(true);
            }
            let fork = head.fork.clone();
            self.store
                .delete(&head_key(&self.config.root, &name))
                .await
                .map_err(object_error)?;
            self.forget_head(&name);
            self.forget_notifier(&name);
            let _ = self.spool.remove(&name);
            if let Some(fork) = fork {
                self.decrement_refs(&fork.source, 0).await?;
            }
            self.notify(&name);
            Ok(false)
        })
    }

    #[tracing::instrument(level = "debug", skip_all, fields(prefix = %prefix.as_str()))]
    fn list(&self, prefix: &StreamName) -> VfsFuture<'_, Vec<VfsEntry>> {
        let prefix = prefix.clone();
        Box::pin(async move {
            let mut entries = Vec::new();
            for name in self.list_stream_names().await? {
                if !name.as_str().starts_with(prefix.as_str()) {
                    continue;
                }
                match self.combined_meta(&name, false).await {
                    Ok((meta, _version, _head)) => entries.push(VfsEntry { name, meta }),
                    Err(VfsError::Gone | VfsError::NotFound) => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(entries)
        })
    }

    fn notifier(&self, name: &StreamName) -> Option<NotifierRef> {
        Some(self.notifier(name))
    }
}

impl ObjectVfs {
    async fn increment_refs(&self, name: &StreamName, delta: u32) -> Result<(), VfsError> {
        let mut attempt = 0_u32;
        loop {
            let (head, etag) = self.load_head(name, true, true).await?;
            let mut next = head.clone();
            next.ref_count = if delta > 0 {
                head.ref_count.saturating_add(delta)
            } else {
                head.ref_count.saturating_sub(delta)
            };
            match self.cas_head(name, Some(etag), &next).await {
                Ok(_) => return Ok(()),
                Err(VfsError::Conflict) => {
                    attempt = attempt.saturating_add(1);
                    if attempt > self.config.retry_bound {
                        return Err(VfsError::Conflict);
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn decrement_refs(&self, name: &StreamName, depth: u32) -> Result<(), VfsError> {
        if depth > self.config.max_fork_depth {
            return Err(VfsError::Conflict);
        }
        let (head, etag) = match self.load_head(name, true, true).await {
            Ok(head) => head,
            Err(VfsError::NotFound) => return Ok(()),
            Err(error) => return Err(error),
        };
        if head.ref_count == 0 {
            return Ok(());
        }
        let mut next = head.clone();
        next.ref_count = head.ref_count.saturating_sub(1);
        match self.cas_head(name, Some(etag), &next).await {
            Ok(_) => {}
            Err(VfsError::Conflict) => {
                return Box::pin(self.decrement_refs(name, depth + 1)).await;
            }
            Err(error) => return Err(error),
        }
        if head.soft_deleted && next.ref_count == 0 {
            let fork = head.fork.clone();
            self.store
                .delete(&head_key(&self.config.root, name))
                .await
                .map_err(object_error)?;
            self.forget_head(name);
            self.forget_notifier(name);
            if let Some(fork) = fork {
                Box::pin(self.decrement_refs(&fork.source, depth + 1)).await?;
            }
        }
        Ok(())
    }
}
