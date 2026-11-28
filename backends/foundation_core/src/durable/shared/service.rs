//! The transport-neutral protocol service.
//!
//! `DurableService` wraps a driver and applies the protocol limits. It has no HTTP, SSE, `ETag` or
//! valtron type. The transport adapters and the valtron layer call it.

use super::{
    AppendOutcome, AppendRequest, Chunk, CloseRequest, CreateOutcome, CreateSpec, DeleteOutcome,
    DurableDriver, DurableStreamStore, ForkSpec, NotifierRef, ReadLimit, ReadPage, ReadRequest,
    StoreError, StreamHead, StreamName,
};

/// The protocol limits. Every value is configuration, never a constant.
#[derive(Clone, Copy, Debug)]
pub struct DurableLimits {
    /// The maximum bytes in one read page.
    pub page_bytes: usize,
    /// The maximum chunks in one read page.
    pub page_chunks: usize,
    /// The long-poll timeout in milliseconds.
    pub long_poll_timeout_ms: u64,
    /// The longest a Server-Sent Events (SSE) connection lives, in milliseconds.
    pub sse_lifetime_ms: u64,
    /// The largest request body the transport adapter reads, in bytes.
    pub max_body_bytes: usize,
}

impl Default for DurableLimits {
    fn default() -> Self {
        Self {
            // 1 MiB: the nginx `client_max_body_size` default, and a safe page size for a
            // streaming reader.
            page_bytes: 1024 * 1024,
            // 256 messages: a small batch that keeps a page bounded.
            page_chunks: 256,
            // 30 s: the Go `net/http` server default read/write idle expectation.
            long_poll_timeout_ms: 30_000,
            // 60 s: the protocol guidance, so a content delivery network (CDN) can collapse
            // reconnects.
            sse_lifetime_ms: 60_000,
            // 8 MiB: the wasm bridge default, below the Cloudflare 100 MB cap and above the
            // nginx 1 MiB default.
            max_body_bytes: 8 * 1024 * 1024,
        }
    }
}

impl DurableLimits {
    /// Set the read page byte limit.
    #[must_use]
    pub fn with_page_bytes(mut self, bytes: usize) -> Self {
        self.page_bytes = bytes;
        self
    }

    /// Set the read page chunk limit.
    #[must_use]
    pub fn with_page_chunks(mut self, chunks: usize) -> Self {
        self.page_chunks = chunks;
        self
    }

    /// Set the long-poll timeout in milliseconds.
    #[must_use]
    pub fn with_long_poll_timeout_ms(mut self, milliseconds: u64) -> Self {
        self.long_poll_timeout_ms = milliseconds;
        self
    }

    /// Set the Server-Sent Events (SSE) connection lifetime in milliseconds.
    #[must_use]
    pub fn with_sse_lifetime_ms(mut self, milliseconds: u64) -> Self {
        self.sse_lifetime_ms = milliseconds;
        self
    }

    /// Set the largest request body the transport adapter reads, in bytes.
    #[must_use]
    pub fn with_max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = bytes;
        self
    }
}

/// The transport-neutral protocol service.
pub struct DurableService<D: DurableDriver> {
    driver: D,
    limits: DurableLimits,
}

impl<D: DurableDriver> DurableService<D> {
    /// Build the service on a driver.
    #[must_use]
    pub fn new(driver: D) -> Self {
        Self {
            driver,
            limits: DurableLimits::default(),
        }
    }

    /// Build the service with explicit limits.
    #[must_use]
    pub fn with_limits(driver: D, limits: DurableLimits) -> Self {
        Self { driver, limits }
    }

    /// Return the driver.
    #[must_use]
    pub fn driver(&self) -> &D {
        &self.driver
    }

    /// Return the limits.
    #[must_use]
    pub fn limits(&self) -> DurableLimits {
        self.limits
    }

    /// Create a stream.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    #[tracing::instrument(
        level = "info",
        skip_all,
        fields(
            stream = %name.as_str(),
            fork = spec.fork.is_some(),
            closed = spec.closed,
            bytes = spec.initial.as_ref().map_or(0, Chunk::len),
        ),
        err
    )]
    pub async fn create(
        &self,
        name: StreamName,
        spec: CreateSpec,
    ) -> Result<CreateOutcome, StoreError> {
        self.driver.store().create(name, spec).await
    }

    /// Return stream metadata.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    #[tracing::instrument(level = "debug", skip_all, fields(stream = %name.as_str()), err)]
    pub async fn head(&self, name: &StreamName) -> Result<StreamHead, StoreError> {
        self.driver.store().head(name).await
    }

    /// Append a chunk.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(
            stream = %name.as_str(),
            bytes = req.chunk.len(),
            producer = ?req.producer.as_ref().map(|producer| producer.id.as_str()),
            epoch = ?req.producer.as_ref().map(|producer| producer.epoch),
            seq = ?req.producer.as_ref().map(|producer| producer.seq),
            close = req.close,
        ),
        err
    )]
    pub async fn append(
        &self,
        name: &StreamName,
        req: AppendRequest,
    ) -> Result<AppendOutcome, StoreError> {
        self.driver.store().append(name, req).await
    }

    /// Read a bounded page.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(
            stream = %name.as_str(),
            offset = ?req.from,
            limit_bytes = req.limit.max_bytes.unwrap_or(self.limits.page_bytes),
            limit_chunks = req.limit.max_chunks.unwrap_or(self.limits.page_chunks),
        ),
        err
    )]
    pub async fn read(&self, name: &StreamName, req: ReadRequest) -> Result<ReadPage, StoreError> {
        let limit = ReadLimit {
            max_bytes: req.limit.max_bytes.or(Some(self.limits.page_bytes)),
            max_chunks: req.limit.max_chunks.or(Some(self.limits.page_chunks)),
        };
        self.driver
            .store()
            .read(
                name,
                ReadRequest {
                    from: req.from,
                    limit,
                },
            )
            .await
    }

    /// Close a stream.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    #[tracing::instrument(
        level = "info",
        skip_all,
        fields(
            stream = %name.as_str(),
            bytes = req.final_chunk.as_ref().map_or(0, Chunk::len),
            producer = ?req.producer.as_ref().map(|producer| producer.id.as_str()),
            epoch = ?req.producer.as_ref().map(|producer| producer.epoch),
            seq = ?req.producer.as_ref().map(|producer| producer.seq),
        ),
        err
    )]
    pub async fn close(
        &self,
        name: &StreamName,
        req: CloseRequest,
    ) -> Result<AppendOutcome, StoreError> {
        self.driver.store().close(name, req).await
    }

    /// Delete a stream.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    #[tracing::instrument(level = "info", skip_all, fields(stream = %name.as_str()), err)]
    pub async fn delete(&self, name: &StreamName) -> Result<DeleteOutcome, StoreError> {
        self.driver.store().delete(name).await
    }

    /// Create a fork.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
    #[tracing::instrument(
        level = "info",
        skip_all,
        fields(
            stream = %name.as_str(),
            source = %spec.source.as_str(),
            at = ?spec.at,
        ),
        err
    )]
    pub async fn fork(
        &self,
        name: StreamName,
        spec: ForkSpec,
    ) -> Result<CreateOutcome, StoreError> {
        self.driver.store().fork(name, spec).await
    }

    /// Return the append notifier.
    #[must_use]
    pub fn notifier(&self, name: &StreamName) -> Option<NotifierRef> {
        self.driver.store().notifier(name)
    }
}
