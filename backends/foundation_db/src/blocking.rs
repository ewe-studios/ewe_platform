//! `Blocking<S>` — the sync boundary over the async storage traits.
//!
//! WHY: every I/O-backed store in this crate implements only the `Async*`
//! traits ([`AsyncKeyValueStore`], [`AsyncBlobStore`], [`AsyncQueryStore`],
//! [`AsyncRateLimiterStore`], [`AsyncDocumentStore`]). Code that is still
//! synchronous needs a way to call them without each backend carrying a sync
//! twin that blocks on its own future (which spins forever on Cloudflare
//! Workers).
//!
//! WHAT: [`Blocking<S>`] wraps any async store and implements the matching
//! sync trait ([`KeyValueStore`], [`BlobStore`], [`QueryStore`],
//! [`RateLimiterStore`], [`DocumentStore`]; [`StorageCleanup`] follows from
//! `QueryStore`) by waiting for the async call on the valtron executor:
//!
//! ```ignore
//! use foundation_db::{Blocking, KeyValueStore, TursoStorage};
//!
//! let store = Blocking::new(TursoStorage::default());
//! store.set("k", "v")?;
//! ```
//!
//! HOW: each sync method moves owned copies of its arguments and a clone of
//! the shared `Arc<S>` into a `'static` future. The future is polled once
//! first: a store that does no I/O (e.g. the in-memory backends) is ready at
//! once and needs no executor. Otherwise it is handed to valtron — single
//! values through `sync_collect_one(from_future(..))` (what `.block()` does,
//! but returning a `StorageError` instead of panicking when no executor is
//! running), streams through `.blocking()`, pulled one item at a time.
//!
//! This module only exists where valtron can block
//! ([`foundation_core::valtron::BLOCKING_BOUNDARY`]): native, and wasm hosts
//! that are not JS. On JS-hosted wasm (Cloudflare Workers, browsers) blocking
//! would freeze the isolate's only thread, so there callers `.await` the async
//! traits directly.
//!
//! Don't use `Blocking` from code that already runs inside a valtron task:
//! waiting there ties up the worker. In async code, call the `Async*` traits.

use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use foundation_core::valtron::{from_future, sync_collect_one, BlockingStreamExt, Stream};
use serde::{de::DeserializeOwned, Serialize};

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncDocumentStore, AsyncKeyValueStore, AsyncQueryStore,
    AsyncRateLimiterStore, BlobStore, DataValue, Document, DocumentStore, KeyValueStore,
    PromotableDocument, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};

/// A sync view of an async store: implements each sync storage trait for
/// `Blocking<S>` where `S` implements the async one.
///
/// Cheap to clone (the store sits behind an `Arc`). [`Deref`]s to the inner
/// store, so its async methods stay reachable.
pub struct Blocking<S> {
    store: Arc<S>,
}

impl<S> Blocking<S> {
    /// Wrap a store.
    #[must_use]
    pub fn new(store: S) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// Wrap a store that is already shared.
    #[must_use]
    pub fn from_arc(store: Arc<S>) -> Self {
        Self { store }
    }

    /// The wrapped store.
    #[must_use]
    pub fn inner(&self) -> &S {
        &self.store
    }

    /// A shared handle to the wrapped store (for passing to async code).
    #[must_use]
    pub fn shared(&self) -> Arc<S> {
        Arc::clone(&self.store)
    }

    /// Unwrap into the shared store.
    #[must_use]
    pub fn into_inner(self) -> Arc<S> {
        self.store
    }
}

impl<S> Clone for Blocking<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
        }
    }
}

impl<S: Default> Default for Blocking<S> {
    fn default() -> Self {
        Self::new(S::default())
    }
}

impl<S> From<S> for Blocking<S> {
    fn from(store: S) -> Self {
        Self::new(store)
    }
}

impl<S> Deref for Blocking<S> {
    type Target = S;

    fn deref(&self) -> &S {
        &self.store
    }
}

impl<S> AsRef<S> for Blocking<S> {
    fn as_ref(&self) -> &S {
        &self.store
    }
}

impl<S> core::fmt::Debug for Blocking<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Blocking")
            .field("store", &core::any::type_name::<S>())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Waiting helpers
// ---------------------------------------------------------------------------

fn executor_error(e: impl core::fmt::Display) -> StorageError {
    StorageError::Backend(format!("Blocking: can't wait on the valtron executor: {e}"))
}

/// Wait for a single-value future: poll it once (a store with no I/O is ready
/// at once and needs no executor), then let valtron drive the rest.
fn wait<T, F>(future: F) -> StorageResult<T>
where
    F: Future<Output = StorageResult<T>> + Send + 'static,
    T: Send + 'static,
{
    let mut future = Box::pin(future);
    if let Poll::Ready(out) = future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        return out;
    }
    sync_collect_one(from_future(future)).map_err(executor_error)?
}

#[cfg(not(target_family = "wasm"))]
type BoxedAsyncStream<U> = Pin<Box<dyn futures_core::Stream<Item = U> + Send>>;
// `AsyncStorageItemStream` is not `Send` on wasm (single-threaded hosts).
#[cfg(target_family = "wasm")]
type BoxedAsyncStream<U> = Pin<Box<dyn futures_core::Stream<Item = U>>>;

#[cfg(not(target_family = "wasm"))]
type BoxedIter<U> = Box<dyn Iterator<Item = U> + Send>;
#[cfg(target_family = "wasm")]
type BoxedIter<U> = Box<dyn Iterator<Item = U>>;

/// A sync iterator over an async stream. Items that are already available are
/// taken without an executor; at the first `Pending` the rest of the stream
/// moves to valtron (`.blocking()`), which then yields one item per `next()`.
/// `on_error` turns an executor failure into a final item.
struct WaitIter<U> {
    fast: Option<BoxedAsyncStream<U>>,
    slow: Option<BoxedIter<U>>,
    on_error: fn(StorageError) -> U,
}

impl<U: Send + 'static> WaitIter<U> {
    fn new(stream: BoxedAsyncStream<U>, on_error: fn(StorageError) -> U) -> Self {
        Self {
            fast: Some(stream),
            slow: None,
            on_error,
        }
    }
}

impl<U: Send + 'static> Iterator for WaitIter<U> {
    type Item = U;

    fn next(&mut self) -> Option<U> {
        if let Some(slow) = self.slow.as_mut() {
            return slow.next();
        }
        let stream = self.fast.as_mut()?;
        match stream
            .as_mut()
            .poll_next(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(Some(item)) => Some(item),
            Poll::Ready(None) => {
                self.fast = None;
                None
            }
            Poll::Pending => {
                let stream = self.fast.take()?;
                match stream.blocking() {
                    Ok(iter) => self.slow.insert(Box::new(iter)).next(),
                    Err(e) => Some((self.on_error)(executor_error(e))),
                }
            }
        }
    }
}

/// Open a `'static` stream with `open`, failing fast when opening fails (the
/// sync traits report open errors as `Err`, not as a first stream item).
fn wait_stream<T, Fut, St>(open: Fut) -> StorageResult<StorageItemStream<'static, T>>
where
    T: Send + 'static,
    Fut: Future<Output = StorageResult<St>> + Send + 'static,
    St: futures_core::Stream<Item = StorageResult<T>> + Send + 'static,
{
    let stream: BoxedAsyncStream<StorageResult<T>> = Box::pin(wait(open)?);
    Ok(Box::new(WaitIter::new(stream, Err).map(Stream::Next)))
}

/// What [`wait_borrowed_stream`]'s generator yields: first whether the stream
/// opened, then its items.
enum Event<T> {
    Opened,
    OpenFailed(StorageError),
    Item(StorageResult<T>),
}

/// Like [`wait_stream`], for streams that borrow the store (the document
/// scans return `AsyncStorageItemStream<'_, V>`). `open` takes the store by
/// reference; a generator that owns the store's `Arc` opens and drains the
/// stream inside itself, so the whole thing is `'static`.
fn wait_borrowed_stream<S, T, F>(store: Arc<S>, open: F) -> StorageResult<StorageItemStream<'static, T>>
where
    S: Send + Sync + 'static,
    T: Send + 'static,
    F: for<'a> FnOnce(
            &'a S,
        ) -> Pin<
            Box<dyn Future<Output = StorageResult<crate::AsyncStorageItemStream<'a, T>>> + Send + 'a>,
        > + Send
        + 'static,
{
    let events: BoxedAsyncStream<Event<T>> = Box::pin(async_stream::stream! {
        match open(&store).await {
            Ok(mut stream) => {
                yield Event::Opened;
                while let Some(item) = futures_lite::StreamExt::next(&mut stream).await {
                    yield Event::Item(item);
                }
            }
            Err(e) => yield Event::OpenFailed(e),
        }
    });
    let mut events = WaitIter::new(events, Event::OpenFailed);
    match events.next() {
        Some(Event::Opened) => {}
        Some(Event::OpenFailed(e)) => return Err(e),
        Some(Event::Item(_)) | None => {
            return Err(StorageError::Backend(
                "Blocking: document scan ended before it opened".to_string(),
            ))
        }
    }
    Ok(Box::new(events.filter_map(|event| match event {
        Event::Item(item) => Some(Stream::Next(item)),
        Event::OpenFailed(e) => Some(Stream::Next(Err(e))),
        Event::Opened => None,
    })))
}

// ---------------------------------------------------------------------------
// Sync trait impls
// ---------------------------------------------------------------------------

impl<S: AsyncKeyValueStore + 'static> KeyValueStore for Blocking<S> {
    fn get<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.get_async::<V>(&key).await })
    }

    fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.set_async(&key, value).await })
    }

    fn delete(&self, key: &str) -> StorageResult<()> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.delete_async(&key).await })
    }

    fn exists(&self, key: &str) -> StorageResult<bool> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.exists_async(&key).await })
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let (store, prefix) = (self.shared(), prefix.map(str::to_owned));
        wait_stream(async move { store.list_keys_async(prefix.as_deref()).await })
    }
}

impl<S: AsyncBlobStore + 'static> BlobStore for Blocking<S> {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let (store, key, data) = (self.shared(), key.to_owned(), data.to_vec());
        wait(async move { store.put_blob_async(&key, &data).await })
    }

    fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.get_blob_async(&key).await })
    }

    fn delete_blob(&self, key: &str) -> StorageResult<()> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.delete_blob_async(&key).await })
    }

    fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.blob_exists_async(&key).await })
    }
}

impl<S: AsyncQueryStore + 'static> QueryStore for Blocking<S> {
    fn query(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        let (store, sql, params) = (self.shared(), sql.to_owned(), params.to_vec());
        wait_stream(async move { store.query_async(&sql, &params).await })
    }

    fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        let (store, sql, params) = (self.shared(), sql.to_owned(), params.to_vec());
        wait(async move { store.execute_async(&sql, &params).await })
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<()> {
        let (store, sql) = (self.shared(), sql.to_owned());
        wait(async move { store.execute_batch_async(&sql).await })
    }
}

impl<S: AsyncRateLimiterStore + 'static> RateLimiterStore for Blocking<S> {
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move {
            store
                .check_rate_limit_async(&key, max_count, window_seconds)
                .await
        })
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<u32> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.record_rate_limit_async(&key).await })
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<()> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.reset_rate_limit_async(&key).await })
    }
}

impl<S: AsyncDocumentStore + 'static> DocumentStore for Blocking<S> {
    fn append<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.append_async(&key, content).await })
    }

    fn append_with_id<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        let (store, key, doc_id) = (self.shared(), key.to_owned(), doc_id.to_owned());
        wait(async move { store.append_with_id_async(&key, &doc_id, content).await })
    }

    fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let key = key.to_owned();
        wait_borrowed_stream(self.shared(), move |store: &S| {
            Box::pin(async move { store.scan_async::<V>(&key, limit).await })
        })
    }

    fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let key = key.to_owned();
        wait_borrowed_stream(self.shared(), move |store: &S| {
            Box::pin(async move { store.scan_all_async::<V>(&key).await })
        })
    }

    fn scan_from<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>> {
        let (key, from_id) = (key.to_owned(), from_id.to_owned());
        wait_borrowed_stream(self.shared(), move |store: &S| {
            Box::pin(async move { store.scan_from_async::<V>(&key, &from_id, limit).await })
        })
    }

    fn append_promotable<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.append_promotable_async(&key, content).await })
    }

    fn append_promotable_with_id<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        let (store, key, doc_id) = (self.shared(), key.to_owned(), doc_id.to_owned());
        wait(async move {
            store
                .append_promotable_with_id_async(&key, &doc_id, content)
                .await
        })
    }

    fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.scan_documents_async(&key, limit).await })
    }

    fn scan_documents_from(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        let (store, key, from_id) = (self.shared(), key.to_owned(), from_id.to_owned());
        wait(async move { store.scan_documents_from_async(&key, &from_id, limit).await })
    }

    fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        let (store, key, doc_id) = (self.shared(), key.to_owned(), doc_id.to_owned());
        wait(async move { store.delete_async(&key, &doc_id).await })
    }

    fn delete_all(&self, key: &str) -> StorageResult<u64> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.delete_all_async(&key).await })
    }

    fn count(&self, key: &str) -> StorageResult<u64> {
        let (store, key) = (self.shared(), key.to_owned());
        wait(async move { store.count_async(&key).await })
    }
}
