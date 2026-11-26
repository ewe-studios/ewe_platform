//! The transport-neutral protocol service.
//!
//! `DurableService` wraps a driver and applies the protocol limits. It has no HTTP, SSE, ETag or
//! valtron type. The transport adapters and the valtron layer call it.

use super::{
    AppendOutcome, AppendRequest, CloseRequest, CreateOutcome, CreateSpec, DeleteOutcome,
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
        }
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
    pub async fn head(&self, name: &StreamName) -> Result<StreamHead, StoreError> {
        self.driver.store().head(name).await
    }

    /// Append a chunk.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
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
    pub async fn delete(&self, name: &StreamName) -> Result<DeleteOutcome, StoreError> {
        self.driver.store().delete(name).await
    }

    /// Create a fork.
    ///
    /// # Errors
    ///
    /// Returns a [`StoreError`] from the driver.
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
