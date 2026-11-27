//! The Cloudflare R2 durable stream virtual file system (VFS).
//!
//! R2 is S3-compatible. The R2 VFS is the shared object-store core with R2's
//! explicit settings: region `auto`, path-style addressing and CRC32
//! checksums. The constructor rejects any other region, because R2 signs with
//! `auto` only. A host may still test another region with a local emulator by
//! calling [`R2Settings::with_region`] and reading the warning in the feature
//! document; the default and the recommended value is `auto`.

use crate::shared::vfs::durable::vfs::{
    CompactionReport, FlushReport, LifecycleReport, ObjectVfs, ProbeReport, ProviderCapabilities,
};
use crate::shared::vfs::durable::{ObjectStore, ObjectVfsConfig, Provider, ProviderSettings};
use foundation_context::Ctx;
use foundation_core::durable::{
    DurableStreamVfs, NotifierRef, StreamName, VfsEntry, VfsError, VfsFuture, VfsMeta, VfsOffset,
};
use std::sync::Arc;

/// The R2 region value. R2 signs with `auto`.
pub const R2_REGION_AUTO: &str = "auto";

/// The explicit R2 settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct R2Settings {
    /// The signing region. `auto` is the only R2 value.
    pub region: String,
    /// True for path-style addressing.
    pub path_style: bool,
    /// True when the client sends a CRC32 checksum.
    pub crc32: bool,
}

impl Default for R2Settings {
    fn default() -> Self {
        Self {
            region: R2_REGION_AUTO.to_string(),
            path_style: true,
            crc32: true,
        }
    }
}

impl R2Settings {
    /// Set the region.
    ///
    /// The R2 service ignores a different value, but it signs with `auto`. A
    /// local emulator can use another value.
    #[must_use]
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    /// Set path-style addressing. R2 needs it for a custom endpoint.
    #[must_use]
    pub fn with_path_style(mut self, path_style: bool) -> Self {
        self.path_style = path_style;
        self
    }

    /// Set the CRC32 checksum.
    #[must_use]
    pub fn with_crc32(mut self, crc32: bool) -> Self {
        self.crc32 = crc32;
        self
    }

    fn provider(self) -> ProviderSettings {
        ProviderSettings {
            provider: Provider::R2,
            region: self.region,
            path_style: self.path_style,
            crc32: self.crc32,
        }
    }
}

/// A durable stream VFS over an R2 object store.
pub struct R2Vfs {
    inner: ObjectVfs,
    settings: R2Settings,
}

impl R2Vfs {
    /// Build an R2 VFS over `store`.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Unsupported`] when the configuration names no spool
    /// root, or an input/output error when the spool cannot be created.
    pub fn new(
        store: Arc<dyn ObjectStore>,
        ctx: &Ctx,
        config: ObjectVfsConfig,
        settings: R2Settings,
    ) -> Result<Self, VfsError> {
        let inner = ObjectVfs::new(store, ctx, config, settings.clone().provider())?;
        Ok(Self { inner, settings })
    }

    /// Build an R2 VFS with the explicit R2 defaults: region `auto`,
    /// path-style and CRC32.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::new`].
    pub fn with_defaults(
        store: Arc<dyn ObjectStore>,
        ctx: &Ctx,
        config: ObjectVfsConfig,
    ) -> Result<Self, VfsError> {
        Self::new(store, ctx, config, R2Settings::default())
    }

    /// Return the R2 settings.
    #[must_use]
    pub fn settings(&self) -> &R2Settings {
        &self.settings
    }

    /// Return the VFS core.
    #[must_use]
    pub fn inner(&self) -> &ObjectVfs {
        &self.inner
    }

    /// Return the provider capability record. The record states whether the
    /// store honours conditional writes.
    #[must_use]
    pub fn provider_capabilities(&self) -> ProviderCapabilities {
        self.inner.provider_capabilities()
    }

    /// Probe the store for conditional write support.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the probe cannot complete.
    pub async fn probe(&self) -> Result<ProbeReport, VfsError> {
        self.inner.probe().await
    }

    /// Flush every eligible stream.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a PUT or a head CAS fails.
    pub async fn flush(&self) -> Result<FlushReport, VfsError> {
        self.inner.flush().await
    }

    /// Flush one stream now.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a PUT or a head CAS fails.
    pub async fn flush_stream(&self, name: &StreamName) -> Result<FlushReport, VfsError> {
        self.inner.flush_stream(name).await
    }

    /// Run the object lifecycle pass.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a LIST or a delete fails.
    pub async fn gc(&self, only: Option<&StreamName>) -> Result<LifecycleReport, VfsError> {
        self.inner.gc(only).await
    }

    /// Compact small segments.
    ///
    /// # Errors
    ///
    /// Returns a storage error when a read, a PUT or the head CAS fails.
    pub async fn compact(&self, name: &StreamName) -> Result<CompactionReport, VfsError> {
        self.inner.compact(name).await
    }
}

impl DurableStreamVfs for R2Vfs {
    fn create(&self, name: &StreamName, meta: VfsMeta) -> VfsFuture<'_, bool> {
        self.inner.create(name, meta)
    }

    fn append(&self, name: &StreamName, bytes: &[u8]) -> VfsFuture<'_, VfsOffset> {
        self.inner.append(name, bytes)
    }

    fn append_with_meta(
        &self,
        name: &StreamName,
        bytes: &[u8],
        meta: VfsMeta,
        expected_version: u64,
    ) -> VfsFuture<'_, VfsOffset> {
        self.inner
            .append_with_meta(name, bytes, meta, expected_version)
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
