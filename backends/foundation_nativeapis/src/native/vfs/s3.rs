//! The Amazon Simple Storage Service (S3) durable stream virtual file system
//! (VFS).
//!
//! The S3 VFS is the shared object-store core with the S3 provider settings.
//! The transport is injected as an [`ObjectStore`]. The client in
//! `object_http.rs` implements the contract over S3 REST requests.
//!
//! Settings: region `us-east-1` by default, virtual-host addressing and no
//! CRC32 checksum. Set path-style addressing for a local server. A host may
//! change every value.

use crate::shared::vfs::durable::vfs::{
    CompactionReport, FlushReport, LifecycleReport, ObjectVfs, ProbeReport, ProviderCapabilities,
};
use crate::shared::vfs::durable::{
    ObjectLayout, ObjectStore, ObjectVfsConfig, Provider, ProviderSettings,
};
use foundation_context::Ctx;
use foundation_core::durable::{
    DurableStreamVfs, NotifierRef, StreamName, VfsEntry, VfsError, VfsFuture, VfsMeta, VfsOffset,
};
use std::sync::Arc;

/// The explicit S3 settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct S3Settings {
    /// The signing region.
    pub region: String,
    /// True for path-style addressing (`endpoint/bucket/key`).
    pub path_style: bool,
    /// True when the client sends a CRC32 checksum.
    pub crc32: bool,
}

impl Default for S3Settings {
    fn default() -> Self {
        Self {
            region: "us-east-1".to_string(),
            path_style: false,
            crc32: false,
        }
    }
}

impl S3Settings {
    /// Set the signing region.
    #[must_use]
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    /// Set path-style addressing.
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
            provider: Provider::S3,
            region: self.region,
            path_style: self.path_style,
            crc32: self.crc32,
        }
    }
}

/// A durable stream VFS over an S3 object store.
pub struct S3Vfs {
    inner: ObjectVfs,
    settings: S3Settings,
}

impl S3Vfs {
    /// Build an S3 VFS over `store`.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Unsupported`] when the configuration names no spool
    /// root, or an input/output error when the spool cannot be created.
    pub fn new(
        store: Arc<dyn ObjectStore>,
        ctx: &Ctx,
        config: ObjectVfsConfig,
        settings: S3Settings,
    ) -> Result<Self, VfsError> {
        let inner = ObjectVfs::new(store, ctx, config, settings.clone().provider())?;
        Ok(Self { inner, settings })
    }

    /// Build an S3 VFS with the provider defaults.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::new`].
    pub fn with_defaults(
        store: Arc<dyn ObjectStore>,
        ctx: &Ctx,
        config: ObjectVfsConfig,
    ) -> Result<Self, VfsError> {
        Self::new(store, ctx, config, S3Settings::default())
    }

    /// Return the S3 settings.
    #[must_use]
    pub fn settings(&self) -> &S3Settings {
        &self.settings
    }

    /// Return the VFS core.
    #[must_use]
    pub fn inner(&self) -> &ObjectVfs {
        &self.inner
    }

    /// Return the object layout.
    #[must_use]
    pub fn layout(&self) -> ObjectLayout {
        self.inner.config().layout
    }

    /// Return the provider capability record.
    #[must_use]
    pub fn provider_capabilities(&self) -> ProviderCapabilities {
        self.inner.provider_capabilities()
    }

    /// Probe the store.
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

impl DurableStreamVfs for S3Vfs {
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
