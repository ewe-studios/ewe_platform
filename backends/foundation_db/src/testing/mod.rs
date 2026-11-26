//! Fault-injecting storage wrappers for deterministic simulation tests.
//!
//! WHY: a deterministic simulation test (DST) must make storage fail at a
//! named step, then check that the caller gets its typed error and leaves no
//! half-written data.
//!
//! WHAT: [`FaultStep`] (the named steps), [`FaultyDocumentStore`] (a
//! `DocumentStore` wrapper) and [`injected_error`].
//!
//! HOW: each call of the named step increments a counter. The wrapper returns
//! an error when the call number matches `fail_on_call`, and delegates every
//! other call to the inner store.

use std::sync::atomic::{AtomicUsize, Ordering};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncStorageItemStream, Document, DocumentStore, PromotableDocument,
};

/// One named step of a storage store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultStep {
    /// `append`.
    Append,
    /// `append_with_id`.
    AppendWithId,
    /// `append_promotable`.
    AppendPromotable,
    /// `append_promotable_with_id`.
    AppendPromotableWithId,
    /// `scan_documents`.
    ScanDocuments,
    /// `scan_documents_from`.
    ScanDocumentsFrom,
    /// `scan`.
    Scan,
    /// `scan_all`.
    ScanAll,
    /// `scan_from`.
    ScanFrom,
    /// `delete`.
    Delete,
    /// `delete_all`.
    DeleteAll,
    /// `count`.
    Count,
}

impl FaultStep {
    /// The method name, for a log line and an error message.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::AppendWithId => "append_with_id",
            Self::AppendPromotable => "append_promotable",
            Self::AppendPromotableWithId => "append_promotable_with_id",
            Self::ScanDocuments => "scan_documents",
            Self::ScanDocumentsFrom => "scan_documents_from",
            Self::Scan => "scan",
            Self::ScanAll => "scan_all",
            Self::ScanFrom => "scan_from",
            Self::Delete => "delete",
            Self::DeleteAll => "delete_all",
            Self::Count => "count",
        }
    }
}

/// The error that an injected failure gives.
#[must_use]
pub fn injected_error(step: FaultStep, call: usize) -> StorageError {
    StorageError::Backend(format!(
        "injected storage failure at {} call {call}",
        step.name()
    ))
}

/// A `DocumentStore` that fails at one named step.
///
/// `fail_on_call` is 1-based: `Some(2)` fails the second call of the step.
/// `None` fails every call of the step.
pub struct FaultyDocumentStore<D> {
    inner: D,
    step: FaultStep,
    fail_on_call: Option<usize>,
    calls: AtomicUsize,
}

impl<D> std::fmt::Debug for FaultyDocumentStore<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaultyDocumentStore")
            .field("step", &self.step)
            .field("fail_on_call", &self.fail_on_call)
            .field("calls", &self.calls.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl<D> FaultyDocumentStore<D> {
    /// A wrapper that fails the first call of `step`.
    #[must_use]
    pub fn new(inner: D, step: FaultStep) -> Self {
        Self {
            inner,
            step,
            fail_on_call: Some(1),
            calls: AtomicUsize::new(0),
        }
    }

    /// Fail the `fail_on_call`-th call of `step`. `None` fails every call.
    #[must_use]
    pub fn with_fail_on_call(mut self, fail_on_call: Option<usize>) -> Self {
        self.fail_on_call = fail_on_call;
        self
    }

    /// The wrapped store.
    #[must_use]
    pub fn inner(&self) -> &D {
        &self.inner
    }

    /// How many calls of the named step this wrapper has seen.
    #[must_use]
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }

    fn check(&self, step: FaultStep) -> StorageResult<()> {
        if step != self.step {
            return Ok(());
        }
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_on_call.is_none_or(|fail| fail == call) {
            return Err(injected_error(step, call));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl<D: DocumentStore> DocumentStore for FaultyDocumentStore<D> {
    async fn append<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        self.check(FaultStep::Append)?;
        self.inner.append(key, content).await
    }

    async fn append_with_id<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        self.check(FaultStep::AppendWithId)?;
        self.inner.append_with_id(key, doc_id, content).await
    }

    async fn append_promotable<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        self.check(FaultStep::AppendPromotable)?;
        self.inner.append_promotable(key, content).await
    }

    async fn append_promotable_with_id<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        self.check(FaultStep::AppendPromotableWithId)?;
        self.inner
            .append_promotable_with_id(key, doc_id, content)
            .await
    }

    async fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
        self.check(FaultStep::ScanDocuments)?;
        self.inner.scan_documents(key, limit).await
    }

    async fn scan_documents_from(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        self.check(FaultStep::ScanDocumentsFrom)?;
        self.inner.scan_documents_from(key, from_id, limit).await
    }

    async fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        self.check(FaultStep::Scan)?;
        self.inner.scan(key, limit).await
    }

    async fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        self.check(FaultStep::ScanAll)?;
        self.inner.scan_all(key).await
    }

    async fn scan_from<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        self.check(FaultStep::ScanFrom)?;
        self.inner.scan_from(key, from_id, limit).await
    }

    async fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        self.check(FaultStep::Delete)?;
        self.inner.delete(key, doc_id).await
    }

    async fn delete_all(&self, key: &str) -> StorageResult<u64> {
        self.check(FaultStep::DeleteAll)?;
        self.inner.delete_all(key).await
    }

    async fn count(&self, key: &str) -> StorageResult<u64> {
        self.check(FaultStep::Count)?;
        self.inner.count(key).await
    }
}
