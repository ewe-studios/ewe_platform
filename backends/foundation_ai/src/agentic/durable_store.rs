//! A `DocumentStore` over Durable Streams — the session transcript log.
//!
//! WHY: the `foundation_ai` session keeps its ordered record log in a
//! `DocumentStore`. The Durable Streams protocol core gives an append-only log
//! with offsets, forks and bounded reads. This store connects the two, so one
//! session uses one durable stream and a branch uses a stream fork.
//!
//! WHAT: [`DurableDocumentStore`] maps one collection key (a session id) to one
//! durable stream. Each document is one JavaScript Object Notation (JSON)
//! message chunk. The stream offset is the log position. A read replays the
//! chunks in append order. [`DurableDocumentStore::fork`] creates a branch
//! stream from a parent stream.
//!
//! HOW: the host builds the [`DurableStreams`] driver and passes it in, with
//! the [`Ctx`] that holds the effect environment. There is no global. The store
//! runs embedded with no Hypertext Transfer Protocol (HTTP).
//!
//! **Terms.** A **collection key** is the string the `DocumentStore` trait
//! passes as `key`; for a session it is the session id. A **document** is one
//! stored record. A **chunk** is one message in a durable stream. The
//! **namespace** is the stream path prefix; the default is `ai/session`.
//!
//! **Delete.** A durable stream is append-only. Deleting one document rewrites
//! the stream, which is the store's compaction path. The rewrite refuses when a
//! fork still shares the stream history.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, MutexGuard};

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, ContentType, CreateSpec, DurableStream, DurableStreams, ForkSpec,
    ReadFrom, ReadLimit, ReadRequest, StoreError, StreamName, StreamOffset,
};
use foundation_db::traits::{AsyncStorageItemStream, Document, DocumentStore, PromotableDocument};
use foundation_db::{StorageError, StorageResult};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The default stream path prefix.
const DEFAULT_NAMESPACE: &str = "ai/session";

/// Limits for [`DurableDocumentStore`].
///
/// Every value is configuration, not a constant. The defaults follow the
/// protocol service limits in [`foundation_core::durable::DurableLimits`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableDocumentStoreConfig {
    /// The stream path prefix (default `ai/session`).
    pub namespace: String,
    /// The largest byte count in one read page (default 1 MiB, the nginx
    /// `client_max_body_size` default).
    pub page_bytes: usize,
    /// The largest chunk count in one read page (default 256, a small batch
    /// that keeps a page bounded).
    pub page_chunks: usize,
}

impl Default for DurableDocumentStoreConfig {
    fn default() -> Self {
        Self {
            namespace: DEFAULT_NAMESPACE.into(),
            page_bytes: 1024 * 1024,
            page_chunks: 256,
        }
    }
}

impl DurableDocumentStoreConfig {
    /// Set the stream path prefix.
    #[must_use]
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    /// Set the largest byte count in one read page.
    #[must_use]
    pub fn with_page_bytes(mut self, page_bytes: usize) -> Self {
        self.page_bytes = page_bytes;
        self
    }

    /// Set the largest chunk count in one read page.
    #[must_use]
    pub fn with_page_chunks(mut self, page_chunks: usize) -> Self {
        self.page_chunks = page_chunks;
        self
    }
}

/// One document as it is stored in a stream chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredDocument {
    /// The stable document id.
    id: String,
    /// The document content, as JSON.
    content: Value,
    /// The document metadata.
    #[serde(default)]
    metadata: Value,
    /// The promoted title column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    /// The promoted summary column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    /// The promoted record type column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    record_type: Option<String>,
}

impl StoredDocument {
    /// Build a document from its parts.
    fn new(
        id: String,
        content: Value,
        title: Option<String>,
        summary: Option<String>,
        record_type: Option<String>,
    ) -> Self {
        Self {
            id,
            content,
            metadata: Value::Object(serde_json::Map::new()),
            title,
            summary,
            record_type,
        }
    }

    /// Encode the document as one JSON message.
    fn encode(&self) -> StorageResult<Vec<u8>> {
        serde_json::to_vec(self).map_err(|error| StorageError::Serialization(error.to_string()))
    }

    /// Decode one stream chunk.
    fn decode(bytes: &[u8]) -> StorageResult<Self> {
        serde_json::from_slice(bytes)
            .map_err(|error| StorageError::Deserialization(error.to_string()))
    }

    /// Return the public `Document` view.
    fn into_document(self) -> StorageResult<Document> {
        let content = serde_json::to_string(&self.content)
            .map_err(|error| StorageError::Serialization(error.to_string()))?;
        Ok(Document::new(
            self.id,
            content,
            self.metadata,
            self.title,
            self.summary,
            self.record_type,
        ))
    }

    /// Deserialize the content into `V`.
    fn into_value<V: DeserializeOwned>(self) -> StorageResult<V> {
        serde_json::from_value(self.content)
            .map_err(|error| StorageError::Deserialization(error.to_string()))
    }
}

/// The lazy stream state for [`DocumentStore::scan_all`] and
/// [`DocumentStore::scan_from`].
struct PageState<'a> {
    /// The store that owns the stream.
    store: &'a DurableDocumentStore,
    /// The stream to read.
    name: StreamName,
    /// The next read position.
    from: ReadFrom,
    /// Documents read from the stream but not yielded yet.
    pending: VecDeque<StoredDocument>,
    /// Return only documents with `id >= from_id`.
    from_id: Option<String>,
    /// Documents left to yield. `None` means unlimited.
    remaining: Option<usize>,
    /// True when the stream has no more data.
    done: bool,
}

/// A session transcript store over one durable stream per collection key.
///
/// The store takes the [`DurableStreams`] driver and the [`Ctx`] from the host.
/// The host builds both at its entry point. There is no global.
///
/// [`DocumentStore::append`] and [`DocumentStore::append_promotable`] mint a
/// fresh document id from the `Env` in the [`Ctx`]. A context without one gives
/// a typed error, never a global id source.
pub struct DurableDocumentStore {
    streams: DurableStreams<DurableStream>,
    ctx: Ctx,
    config: DurableDocumentStoreConfig,
    content_type: ContentType,
    /// Live document ids per collection key. The first write of a key fills it
    /// by replaying the stream, so an idempotent re-append is a no-op even
    /// after a reopen.
    index: Mutex<HashMap<String, HashSet<String>>>,
}

impl DurableDocumentStore {
    /// Build a store over a durable stream driver with the default limits.
    #[must_use]
    pub fn new(streams: DurableStreams<DurableStream>, ctx: Ctx) -> Self {
        Self::with_config(streams, ctx, DurableDocumentStoreConfig::default())
    }

    /// Build a store with explicit limits.
    ///
    /// # Panics
    ///
    /// Panics when the store cannot build its fixed `application/json`
    /// content type. That value is a literal, so the panic cannot happen.
    #[must_use]
    pub fn with_config(
        streams: DurableStreams<DurableStream>,
        ctx: Ctx,
        config: DurableDocumentStoreConfig,
    ) -> Self {
        let content_type = ContentType::parse("application/json")
            .expect("application/json is a valid content type");
        Self {
            streams,
            ctx,
            config,
            content_type,
            index: Mutex::new(HashMap::new()),
        }
    }

    /// The stream a collection key maps to.
    ///
    /// The key is percent-encoded, so any string is a valid path segment.
    /// # Errors
    ///
    /// Returns [`StorageError::Backend`] when the key is empty or the
    /// namespace is not a valid stream path.
    pub fn stream_name(&self, key: &str) -> StorageResult<StreamName> {
        let segment = encode_segment(key);
        if segment.is_empty() {
            return Err(StorageError::Backend(
                "durable document store: the collection key is empty".into(),
            ));
        }
        let path = format!("{}/{segment}", self.config.namespace);
        StreamName::parse(&path).map_err(|error| {
            StorageError::Backend(format!(
                "durable document store: bad stream path {path:?}: {error}"
            ))
        })
    }

    /// Create a branch stream from a parent collection key.
    ///
    /// A read of the branch returns the parent prefix, then the branch's own
    /// appends. `at` is a parent stream offset; `None` forks at the parent
    /// tail. Use [`tail_offset`](Self::tail_offset) to read that offset.
    /// # Errors
    ///
    /// Returns [`StorageError::Backend`] when the parent is missing, the
    /// offset is invalid, or the branch already exists with a different
    /// configuration.
    #[tracing::instrument(
        level = "info",
        skip_all,
        fields(stream = %parent, branch = %branch, at = ?at),
        err
    )]
    pub async fn fork(
        &self,
        parent: &str,
        branch: &str,
        at: Option<StreamOffset>,
    ) -> StorageResult<()> {
        let source = self.stream_name(parent)?;
        let target = self.stream_name(branch)?;
        self.streams
            .stream(target.clone())
            .create(CreateSpec {
                content_type: Some(self.content_type.clone()),
                expires: None,
                closed: false,
                initial: None,
                fork: Some(ForkSpec {
                    source,
                    at,
                    sub_offset: 0,
                }),
            })
            .await
            .map_err(|error| Self::error("fork", &target, &error))?;
        self.index_invalidate(branch);
        Ok(())
    }

    /// The current tail offset of a collection key's stream.
    ///
    /// Returns `None` when the stream does not exist.
    /// # Errors
    ///
    /// Returns [`StorageError::Backend`] when the stream head cannot be read.
    #[tracing::instrument(level = "debug", skip_all, fields(stream = %key), err)]
    pub async fn tail_offset(&self, key: &str) -> StorageResult<Option<StreamOffset>> {
        let name = self.stream_name(key)?;
        match self.streams.stream(name).head().await {
            Ok(head) => Ok(Some(head.tail)),
            Err(StoreError::NotFound | StoreError::Gone) => Ok(None),
            Err(error) => Err(Self::error("head", &self.stream_name(key)?, &error)),
        }
    }

    /// Replay a collection from an offset, in append order.
    ///
    /// `from` is a stream offset. `None` replays from the start.
    /// # Errors
    ///
    /// Returns [`StorageError`] when the stream cannot be read or a chunk is
    /// not a stored document.
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %key, offset = ?from),
        err
    )]
    pub async fn replay(
        &self,
        key: &str,
        from: Option<StreamOffset>,
    ) -> StorageResult<Vec<Document>> {
        let name = self.stream_name(key)?;
        let start = from.map_or(ReadFrom::Start, ReadFrom::Offset);
        let documents = self.read_entries(&name, start).await?;
        documents
            .into_iter()
            .map(StoredDocument::into_document)
            .collect()
    }

    // -------------------------------------------------------------------
    // Internal stream operations

    /// Map a core error to a storage error that names the operation.
    fn error(operation: &str, name: &StreamName, error: &StoreError) -> StorageError {
        StorageError::Backend(format!(
            "durable document store: {operation} {}: {error}",
            name.as_str()
        ))
    }

    /// Lock the id index; a poisoned lock is a backend error.
    fn lock_index(&self) -> StorageResult<MutexGuard<'_, HashMap<String, HashSet<String>>>> {
        self.index
            .lock()
            .map_err(|error| StorageError::Backend(format!("id index lock: {error}")))
    }

    /// Forget the cached id set for one key.
    fn index_invalidate(&self, key: &str) {
        if let Ok(mut index) = self.index.lock() {
            index.remove(key);
        }
    }

    /// Record a live id for one key.
    fn index_insert(&self, key: &str, id: &str) {
        if let Ok(mut index) = self.index.lock() {
            index
                .entry(key.to_string())
                .or_default()
                .insert(id.to_string());
        }
    }

    /// Drop a live id for one key.
    fn index_remove(&self, key: &str, id: &str) {
        if let Ok(mut index) = self.index.lock() {
            if let Some(ids) = index.get_mut(key) {
                ids.remove(id);
            }
        }
    }

    /// Report whether a live document id exists, filling the cache on first use.
    async fn index_contains(&self, key: &str, id: &str) -> StorageResult<bool> {
        {
            let index = self.lock_index()?;
            if let Some(ids) = index.get(key) {
                return Ok(ids.contains(id));
            }
        }
        // The cache is cold. Replay the stream once to learn its live ids.
        let name = self.stream_name(key)?;
        let entries = self.read_entries(&name, ReadFrom::Start).await?;
        let ids: HashSet<String> = entries.into_iter().map(|entry| entry.id).collect();
        let mut index = self.lock_index()?;
        let cached = index.entry(key.to_string()).or_default();
        cached.extend(ids);
        Ok(cached.contains(id))
    }

    /// Mint a fresh document id from the injected effect environment.
    fn fresh_id(&self) -> StorageResult<String> {
        let env = self.ctx.require_env().map_err(|error| {
            StorageError::Backend(format!(
                "durable document store: append needs an Env in the Ctx: {error}"
            ))
        })?;
        Ok(env.ids().next_id().to_string())
    }

    /// Create the stream when it does not exist. An existing stream is kept.
    async fn ensure_stream(&self, name: &StreamName) -> StorageResult<()> {
        let handle = self.streams.stream(name.clone());
        match handle.head().await {
            Ok(_) => Ok(()),
            Err(StoreError::NotFound) => handle
                .create(CreateSpec {
                    content_type: Some(self.content_type.clone()),
                    expires: None,
                    closed: false,
                    initial: None,
                    fork: None,
                })
                .await
                .map(|_| ())
                .map_err(|error| Self::error("create", name, &error)),
            Err(error) => Err(Self::error("head", name, &error)),
        }
    }

    /// Append one document to its stream.
    async fn append_document(
        &self,
        key: &str,
        document: StoredDocument,
    ) -> StorageResult<Document> {
        let name = self.stream_name(key)?;
        self.ensure_stream(&name).await?;
        self.append_to_stream(&name, &document).await?;
        self.index_insert(key, &document.id);
        document.into_document()
    }

    /// Append to a stream that already exists.
    async fn append_to_stream(
        &self,
        name: &StreamName,
        document: &StoredDocument,
    ) -> StorageResult<()> {
        let bytes = document.encode()?;
        self.streams
            .stream(name.clone())
            .append(AppendRequest {
                chunk: Chunk::new(bytes),
                content_type: Some(self.content_type.clone()),
                stream_seq: None,
                producer: None,
                close: false,
            })
            .await
            .map(|_| ())
            .map_err(|error| Self::error("append", name, &error))
    }

    /// Read one bounded page.
    ///
    /// A missing or deleted stream reads as an empty, complete page.
    async fn read_page(
        &self,
        name: &StreamName,
        from: &ReadFrom,
    ) -> StorageResult<(Vec<StoredDocument>, StreamOffset, bool)> {
        let request = ReadRequest {
            from: from.clone(),
            limit: ReadLimit::new(Some(self.config.page_bytes), Some(self.config.page_chunks)),
        };
        let page = match self.streams.stream(name.clone()).read(request).await {
            Ok(page) => page,
            Err(StoreError::NotFound | StoreError::Gone) => {
                return Ok((Vec::new(), StreamOffset::from_bytes(Vec::new()), true));
            }
            Err(error) => return Err(Self::error("read", name, &error)),
        };
        let documents = page
            .chunks
            .iter()
            .map(|chunk| StoredDocument::decode(chunk.as_bytes()))
            .collect::<StorageResult<Vec<_>>>()?;
        Ok((documents, page.cursor.next, page.cursor.up_to_date))
    }

    /// Read every document from a position, in append order.
    async fn read_entries(
        &self,
        name: &StreamName,
        start: ReadFrom,
    ) -> StorageResult<Vec<StoredDocument>> {
        let mut from = start;
        let mut documents = Vec::new();
        loop {
            let (mut page, next, up_to_date) = self.read_page(name, &from).await?;
            documents.append(&mut page);
            if up_to_date {
                return Ok(documents);
            }
            let advanced = ReadFrom::Offset(next);
            if advanced == from {
                // A page that makes no progress would loop forever.
                return Err(StorageError::Backend(format!(
                    "durable document store: read of {} made no progress",
                    name.as_str()
                )));
            }
            from = advanced;
        }
    }

    /// Report whether any stream forks this one.
    async fn has_forks(&self, name: &StreamName) -> StorageResult<bool> {
        let prefix = StreamName::parse(&self.config.namespace).map_err(|error| {
            StorageError::Backend(format!(
                "durable document store: bad namespace {:?}: {error}",
                self.config.namespace
            ))
        })?;
        let entries = self
            .streams
            .driver()
            .vfs()
            .list(&prefix)
            .await
            .map_err(|error| {
                StorageError::Backend(format!(
                    "durable document store: list {}: {error}",
                    prefix.as_str()
                ))
            })?;
        Ok(entries.iter().any(|entry| {
            entry
                .meta
                .fork
                .as_ref()
                .is_some_and(|fork| &fork.source == name)
        }))
    }

    /// Rewrite a stream without one document — the compaction path for delete.
    async fn delete_one(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        let name = self.stream_name(key)?;
        let documents = self.read_entries(&name, ReadFrom::Start).await?;
        if !documents.iter().any(|document| document.id == doc_id) {
            return Ok(());
        }
        if self.has_forks(&name).await? {
            return Err(StorageError::Backend(format!(
                "durable document store: refuse to rewrite {}: a fork shares its history",
                name.as_str()
            )));
        }
        let survivors: Vec<StoredDocument> = documents
            .into_iter()
            .filter(|document| document.id != doc_id)
            .collect();
        match self.streams.stream(name.clone()).delete().await {
            Ok(_) | Err(StoreError::NotFound | StoreError::Gone) => {}
            Err(error) => return Err(Self::error("delete", &name, &error)),
        }
        self.ensure_stream(&name).await?;
        for document in &survivors {
            self.append_to_stream(&name, document).await?;
        }
        self.index_remove(key, doc_id);
        Ok(())
    }

    /// Build a lazily-pulled stream over a stream's chunks.
    fn page_stream<V: DeserializeOwned + Send + 'static>(
        &self,
        name: StreamName,
        from: ReadFrom,
        from_id: Option<String>,
        limit: Option<usize>,
    ) -> AsyncStorageItemStream<'_, V> {
        let state = PageState {
            store: self,
            name,
            from,
            pending: VecDeque::new(),
            from_id,
            remaining: limit,
            done: false,
        };
        Box::pin(futures_lite::stream::unfold(
            state,
            |mut state| async move {
                loop {
                    if state.remaining.is_some_and(|remaining| remaining == 0) {
                        return None;
                    }
                    if let Some(document) = state.pending.pop_front() {
                        if let Some(from_id) = &state.from_id {
                            if document.id.as_str() < from_id.as_str() {
                                continue;
                            }
                        }
                        if let Some(remaining) = &mut state.remaining {
                            *remaining -= 1;
                        }
                        let is_error = document.clone().into_value::<V>().is_err();
                        let item = document.into_value::<V>();
                        if is_error {
                            state.done = true;
                        }
                        return Some((item, state));
                    }
                    if state.done {
                        return None;
                    }
                    match state.store.read_page(&state.name, &state.from).await {
                        Ok((documents, next, up_to_date)) => {
                            state.pending.extend(documents);
                            let advanced = ReadFrom::Offset(next);
                            if !up_to_date && advanced == state.from {
                                state.done = true;
                            } else {
                                state.from = advanced;
                                state.done = up_to_date;
                            }
                        }
                        Err(error) => {
                            state.done = true;
                            return Some((Err(error), state));
                        }
                    }
                }
            },
        ))
    }
}

#[async_trait::async_trait]
impl DocumentStore for DurableDocumentStore {
    #[tracing::instrument(level = "debug", skip_all, fields(stream = %key))]
    async fn append<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let id = self.fresh_id()?;
        let content = serde_json::to_value(content)
            .map_err(|error| StorageError::Serialization(error.to_string()))?;
        self.append_document(key, StoredDocument::new(id, content, None, None, None))
            .await
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %key, id = %doc_id)
    )]
    async fn append_with_id<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        if self.index_contains(key, doc_id).await? {
            // Idempotent (F16 E7): a retry keeps the stored document.
            let name = self.stream_name(key)?;
            let existing = self
                .read_entries(&name, ReadFrom::Start)
                .await?
                .into_iter()
                .find(|document| document.id == doc_id)
                .ok_or_else(|| {
                    StorageError::Backend(format!(
                        "durable document store: id {doc_id} indexed but not in {}",
                        name.as_str()
                    ))
                })?;
            return existing.into_document();
        }
        let content = serde_json::to_value(content)
            .map_err(|error| StorageError::Serialization(error.to_string()))?;
        self.append_document(
            key,
            StoredDocument::new(doc_id.to_string(), content, None, None, None),
        )
        .await
    }

    #[tracing::instrument(level = "debug", skip_all, fields(stream = %key))]
    async fn append_promotable<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document> {
        let id = self.fresh_id()?;
        let (title, summary, record_type) =
            (content.title(), content.summary(), content.record_type());
        let content = serde_json::to_value(content)
            .map_err(|error| StorageError::Serialization(error.to_string()))?;
        self.append_document(
            key,
            StoredDocument::new(id, content, title, summary, record_type),
        )
        .await
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %key, id = %doc_id)
    )]
    async fn append_promotable_with_id<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document> {
        if self.index_contains(key, doc_id).await? {
            let name = self.stream_name(key)?;
            let existing = self
                .read_entries(&name, ReadFrom::Start)
                .await?
                .into_iter()
                .find(|document| document.id == doc_id)
                .ok_or_else(|| {
                    StorageError::Backend(format!(
                        "durable document store: id {doc_id} indexed but not in {}",
                        name.as_str()
                    ))
                })?;
            return existing.into_document();
        }
        let (title, summary, record_type) =
            (content.title(), content.summary(), content.record_type());
        let content = serde_json::to_value(content)
            .map_err(|error| StorageError::Serialization(error.to_string()))?;
        self.append_document(
            key,
            StoredDocument::new(doc_id.to_string(), content, title, summary, record_type),
        )
        .await
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %key, limit = limit)
    )]
    async fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
        let name = self.stream_name(key)?;
        let mut documents = self.read_entries(&name, ReadFrom::Start).await?;
        // Newest-first by id. The session ids are scru128, so this is also the
        // newest-first stream order.
        documents.sort_by(|left, right| right.id.cmp(&left.id));
        documents.truncate(limit);
        documents
            .into_iter()
            .map(StoredDocument::into_document)
            .collect()
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %key, from_id = %from_id, limit = limit)
    )]
    async fn scan_documents_from(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>> {
        let name = self.stream_name(key)?;
        let mut documents = self.read_entries(&name, ReadFrom::Start).await?;
        documents.sort_by(|left, right| left.id.cmp(&right.id));
        documents.retain(|document| document.id.as_str() >= from_id);
        if limit > 0 {
            documents.truncate(limit);
        }
        documents
            .into_iter()
            .map(StoredDocument::into_document)
            .collect()
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %key, limit = limit)
    )]
    async fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let name = self.stream_name(key)?;
        let mut documents = self.read_entries(&name, ReadFrom::Start).await?;
        documents.sort_by(|left, right| right.id.cmp(&left.id));
        documents.truncate(limit);
        let items = documents
            .into_iter()
            .map(StoredDocument::into_value::<V>)
            .collect::<Vec<_>>();
        Ok(Box::pin(futures_lite::stream::iter(items)))
    }

    #[tracing::instrument(level = "debug", skip_all, fields(stream = %key))]
    async fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let name = self.stream_name(key)?;
        Ok(self.page_stream::<V>(name, ReadFrom::Start, None, None))
    }

    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(stream = %key, from_id = %from_id, limit = limit)
    )]
    async fn scan_from<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
        let name = self.stream_name(key)?;
        let limit = (limit > 0).then_some(limit);
        Ok(self.page_stream::<V>(name, ReadFrom::Start, Some(from_id.to_string()), limit))
    }

    #[tracing::instrument(
        level = "info",
        skip_all,
        fields(stream = %key, id = %doc_id)
    )]
    async fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
        self.delete_one(key, doc_id).await
    }

    #[tracing::instrument(level = "info", skip_all, fields(stream = %key))]
    async fn delete_all(&self, key: &str) -> StorageResult<u64> {
        let name = self.stream_name(key)?;
        let count = self.read_entries(&name, ReadFrom::Start).await?.len() as u64;
        match self.streams.stream(name.clone()).delete().await {
            Ok(_) | Err(StoreError::NotFound | StoreError::Gone) => {}
            Err(error) => return Err(Self::error("delete", &name, &error)),
        }
        self.index_invalidate(key);
        Ok(count)
    }

    #[tracing::instrument(level = "debug", skip_all, fields(stream = %key))]
    async fn count(&self, key: &str) -> StorageResult<u64> {
        let name = self.stream_name(key)?;
        Ok(self.read_entries(&name, ReadFrom::Start).await?.len() as u64)
    }
}

/// Percent-encode a collection key for one stream path segment.
///
/// Only ASCII letters, digits, `-` and `_` stay as they are. Every other byte
/// becomes `%xx`. A session id is alphanumeric, so it stays readable.
fn encode_segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(hex_digit(byte >> 4));
            out.push(hex_digit(byte & 0x0f));
        }
    }
    out
}

/// A lowercase hexadecimal digit.
fn hex_digit(value: u8) -> char {
    char::from(match value {
        0..=9 => b'0' + value,
        _ => b'a' + (value - 10),
    })
}

impl std::fmt::Debug for DurableDocumentStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DurableDocumentStore")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
