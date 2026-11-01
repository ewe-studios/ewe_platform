//! `MessageApi` — the session's authoritative append-only audit log (F08).
//!
//! WHY: Every interaction must be durably recorded, replayable, and observable
//! in real time — without per-record disk latency stalling the agent loop.
//!
//! WHAT: `Arc<MessageInner>` over an [`DocumentStore`] (F06), a bounded
//! write buffer that the agent loop flushes, and `&self` pub/sub for listeners. Semantic search is optional
//! (gated behind vector-store/embedding providers; the core append/flush/pub-sub
//! works without them).

use std::future::poll_fn;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::Waker;

use concurrent_queue::ConcurrentQueue;
use foundation_compact::SendWrapper;
pub use foundation_core::synca::mpp::Receiver;
use foundation_core::synca::mpp::TrackedBroadcaster;
use foundation_db::traits::{DocumentStore, Document};
use foundation_db::{StorageError, StorageResult};
use futures_lite::StreamExt;
use serde::{Deserialize, Serialize};

use crate::types::{SessionId, SessionRecord};

// ---------------------------------------------------------------------------
// MessageEvent — pub/sub payload

/// Events broadcast to subscribers when records are appended or flushed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageEvent {
    /// A new record was enqueued into the buffer.
    Appended {
        /// The scru128 id of the record.
        id: String,
        /// The variant name for quick filtering
        /// (`"conversation"` / `"working_memory"` / …).
        variant: &'static str,
    },
    /// A batch was flushed to the document store.
    Flushed { count: usize },
    /// An error occurred (e.g., indexing failure — durability is kept regardless).
    Error { msg: String },
}

impl MessageEvent {
    fn variant_name(record: &SessionRecord) -> &'static str {
        match record {
            SessionRecord::Conversation { .. } => "conversation",
            SessionRecord::WorkingMemory { .. } => "working_memory",
            SessionRecord::Observation { .. } => "observation",
            SessionRecord::Reflection { .. } => "reflection",
            SessionRecord::FailedAction { .. } => "failed_action",
            SessionRecord::Summary { .. } => "summary",
            SessionRecord::Retracted { .. } => "retracted",
        }
    }
}

// ---------------------------------------------------------------------------
// MessageApiConfig

/// Limits for a [`MessageApi`].
///
/// WHY: the write buffer and the subscriber channel are both bounded, and
/// their sizes are tuning, not code (limits are configuration).
///
/// WHAT: `flush_threshold` — how many buffered records make
/// [`MessageApi::needs_flush`] true (default 50, the size at which the agent
/// loop writes the buffer out; small enough to keep a crash's loss to a few
/// turns, large enough to batch a turn's records into one flush);
/// `subscriber_capacity` — the per-subscriber event channel capacity
/// (default 256).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageApiConfig {
    /// Buffered records at which a flush is due (default 50).
    pub flush_threshold: usize,
    /// Capacity of each subscriber's event channel (default 256).
    pub subscriber_capacity: usize,
}

impl Default for MessageApiConfig {
    fn default() -> Self {
        Self {
            flush_threshold: 50,
            subscriber_capacity: 256,
        }
    }
}

impl MessageApiConfig {
    /// Set the number of buffered records at which a flush is due.
    #[must_use]
    pub fn with_flush_threshold(mut self, flush_threshold: usize) -> Self {
        self.flush_threshold = flush_threshold;
        self
    }

    /// Set the capacity of each subscriber's event channel.
    #[must_use]
    pub fn with_subscriber_capacity(mut self, subscriber_capacity: usize) -> Self {
        self.subscriber_capacity = subscriber_capacity;
        self
    }
}

// ---------------------------------------------------------------------------
// MessageInner

struct MessageInner<D> {
    session_id: SessionId,
    /// Shared so the session can hand the same store to other components
    /// (e.g. the memory coordinator's audit log) instead of a second copy.
    doc_store: Arc<D>,
    write_buffer: ConcurrentQueue<SessionRecord>,
    broadcaster: TrackedBroadcaster<MessageEvent>,
    /// Buffered records at which a flush is due.
    flush_threshold: usize,
    /// Flush coordination: whether one runs now, and the tasks waiting for it.
    flush: Mutex<FlushState>,
    /// The record being written, or the one the store refused on the last
    /// flush. The next flush writes it first, so a failed (or cancelled)
    /// flush neither loses the record nor reorders the log.
    retry: Mutex<Option<SessionRecord>>,
}

/// Who is flushing right now, and who awaits its completion.
#[derive(Default)]
struct FlushState {
    running: bool,
    waiters: Vec<Waker>,
}

impl<D> MessageInner<D> {
    /// Records not yet written: the buffer plus a record held back by a
    /// failed flush.
    fn unflushed(&self) -> usize {
        let held = usize::from(
            self.retry
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_some(),
        );
        self.write_buffer.len() + held
    }
}

/// Ends a running flush — including when its future is dropped part-way —
/// and wakes every task awaiting it, so a cancelled flush cannot leave later
/// flushes parked forever.
struct FlushGuard<'a>(&'a Mutex<FlushState>);

impl Drop for FlushGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        state.running = false;
        for waiter in std::mem::take(&mut state.waiters) {
            waiter.wake();
        }
    }
}

impl<D: DocumentStore> MessageInner<D> {
    /// Flush all buffered records to the document store.
    ///
    /// The first caller becomes the flusher. A caller that finds a flush
    /// already running awaits its completion (instead of returning `Ok(0)` at
    /// once), so a slow store never makes the agent loop re-enter `Flushing`
    /// in a tight loop. On a store error the failing record is kept for the
    /// next flush (see `retry`), the records behind it stay buffered, and the
    /// running flag is cleared — so a later flush can succeed.
    async fn flush(&self) -> StorageResult<usize> {
        // The guard ends a running flush, but `MutexGuard` is `!Send` and must
        // not live across the awaits below, so the state is read and released
        // in an inner block first.
        let running = {
            let mut state = self.flush.lock().unwrap_or_else(PoisonError::into_inner);
            if state.running {
                true
            } else {
                state.running = true;
                false
            }
        };
        if running {
            return self.wait_for_flush().await;
        }
        let _guard = FlushGuard(&self.flush);
        self.flush_buffered().await
    }

    /// Park until no flush is running, then return `Ok(0)`: the running flush
    /// already drained the buffer, so this caller has nothing left to write.
    fn wait_for_flush(&self) -> impl std::future::Future<Output = StorageResult<usize>> + '_ {
        poll_fn(move |cx| {
            let mut state = self.flush.lock().unwrap_or_else(PoisonError::into_inner);
            if !state.running {
                return std::task::Poll::Ready(Ok(0));
            }
            if !state.waiters.iter().any(|w| w.will_wake(cx.waker())) {
                state.waiters.push(cx.waker().clone());
            }
            std::task::Poll::Pending
        })
    }

    /// The next record to write: a held-back one first, then the buffer.
    /// The record is parked in `retry` until it is written.
    fn next_record(&self) -> Option<SessionRecord> {
        let mut retry = self.retry.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(record) = retry.as_ref() {
            return Some(record.clone());
        }
        let record = self.write_buffer.pop().ok()?;
        *retry = Some(record.clone());
        Some(record)
    }

    async fn flush_buffered(&self) -> StorageResult<usize> {
        let mut count = 0;
        let outcome = loop {
            let Some(record) = self.next_record() else {
                break Ok(count);
            };
            if let Err(error) = self.write(&record).await {
                self.broadcaster.broadcast(MessageEvent::Error {
                    msg: format!("flushing session {}: {error}", self.session_id),
                });
                break Err(error);
            }
            *self.retry.lock().unwrap_or_else(PoisonError::into_inner) = None;
            count += 1;
        };
        if count > 0 {
            self.broadcaster.broadcast(MessageEvent::Flushed { count });
        }
        outcome
    }

    /// Write one record to the store.
    async fn write(&self, record: &SessionRecord) -> StorageResult<()> {
        let key = self.session_id.to_string();
        // append_with_id for conversation records (stable id),
        // append_promotable for memory records (promotes columns).
        match record {
            SessionRecord::Conversation { message } => {
                self.doc_store
                    .append_with_id(&key, &message.id().to_string(), record.clone())
                    .await?;
            }
            SessionRecord::WorkingMemory { id, .. }
            | SessionRecord::Observation { id, .. }
            | SessionRecord::Reflection { id, .. } => {
                // The record's own id is stable across a retry. A second write
                // of the same record is a no-op, so no duplicate is created.
                self.doc_store
                    .append_promotable_with_id(&key, &id.to_string(), record.clone())
                    .await?;
            }
            _ => {
                self.doc_store.append(&key, record.clone()).await?;
            }
        }
        Ok(())
    }

}

// ---------------------------------------------------------------------------
// MessageApi

/// The session's authoritative append-only message log. Cheap to clone
/// (`Arc`-shared) across valtron tasks.
///
/// `append` only buffers; the store is written by [`flush`](Self::flush),
/// which every read (`recent`, `all`, `scan_from`, `clear`) runs first.
///
/// What bounds the buffer: inside a turn, the agent loop awaits a flush as
/// soon as [`needs_flush`](Self::needs_flush) reports the threshold reached,
/// at every context assembly, and when the turn ends. Code that appends
/// outside a running turn (e.g. between turns) bounds it by awaiting
/// `flush()` once `needs_flush()` is true; `AgentSession::end` flushes too.
///
/// # Example
/// ```ignore
/// let api = MessageApi::new(session_id, doc_store);
/// api.append(record);
/// let records = api.recent(10).await?;
/// ```
pub struct MessageApi<D> {
    inner: Arc<MessageInner<D>>,
}

impl<D> Clone for MessageApi<D> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<D> MessageApi<D> {
    /// Create a new `MessageApi` over the given document store, with the
    /// default [`MessageApiConfig`].
    pub fn new(session_id: SessionId, doc_store: D) -> Self {
        Self::with_config(session_id, doc_store, MessageApiConfig::default())
    }

    /// Create over a store that is shared with other components.
    pub fn from_shared(session_id: SessionId, doc_store: Arc<D>) -> Self {
        Self::shared_with_config(session_id, doc_store, MessageApiConfig::default())
    }

    /// Create with explicit limits.
    pub fn with_config(session_id: SessionId, doc_store: D, config: MessageApiConfig) -> Self {
        Self::shared_with_config(session_id, Arc::new(doc_store), config)
    }

    /// [`with_config`](Self::with_config) over a shared store.
    pub fn shared_with_config(
        session_id: SessionId,
        doc_store: Arc<D>,
        config: MessageApiConfig,
    ) -> Self {
        Self {
            inner: Arc::new(MessageInner {
                session_id,
                doc_store,
                write_buffer: ConcurrentQueue::unbounded(),
                broadcaster: TrackedBroadcaster::new(config.subscriber_capacity),
                flush_threshold: config.flush_threshold,
                flush: Mutex::new(FlushState::default()),
                retry: Mutex::new(None),
            }),
        }
    }

    /// Subscribe to message events (appends, flushes, errors).
    #[must_use]
    pub fn subscribe(&self) -> Receiver<MessageEvent> {
        self.inner.broadcaster.subscribe()
    }

    /// Append a record to the session log. Returns the record's scru128 id.
    ///
    /// Buffers only (no I/O); see the type docs for what flushes the buffer.
    #[must_use]
    pub fn append(&self, record: SessionRecord) -> String {
        let id = match &record {
            SessionRecord::Conversation { message } => message.id().to_string(),
            // Memory records are stored under their own id (see `write`), so
            // the pub/sub event id must match that id, not a fresh mint.
            SessionRecord::WorkingMemory { id, .. }
            | SessionRecord::Observation { id, .. }
            | SessionRecord::Reflection { id, .. } => id.to_string(),
            _ => foundation_compact::ids::new_scru128_string(),
        };
        let variant = MessageEvent::variant_name(&record);
        if let Err(error) = self.inner.write_buffer.push(record) {
            // An unbounded queue only refuses a push once closed, and this one
            // is never closed; report it rather than lose the record silently.
            tracing::error!(
                session = %self.inner.session_id,
                %error,
                "message buffer refused a record"
            );
            self.inner.broadcaster.broadcast(MessageEvent::Error {
                msg: format!("buffering a record for session {}: {error}", self.inner.session_id),
            });
            return id;
        }

        self.inner.broadcaster.broadcast(MessageEvent::Appended {
            id: id.clone(),
            variant,
        });

        id
    }

    /// Whether the buffer has reached the configured flush threshold.
    #[must_use]
    pub fn needs_flush(&self) -> bool {
        self.inner.write_buffer.len() >= self.inner.flush_threshold
    }

    /// The configured flush threshold.
    #[must_use]
    pub fn flush_threshold(&self) -> usize {
        self.inner.flush_threshold
    }

    /// Records appended but not yet written to the store (including one held
    /// back by a failed flush).
    #[must_use]
    pub fn unflushed(&self) -> usize {
        self.inner.unflushed()
    }
}

impl<D: DocumentStore> MessageApi<D> {
    /// Flush all buffered records to the document store now.
    ///
    /// Returns the number written. If another flush is already running this
    /// returns `Ok(0)` and leaves the buffer to it.
    /// # Errors
    /// Returns [`StorageError`] if the store refuses a record; that record and
    /// the ones behind it stay buffered for the next flush.
    pub async fn flush(&self) -> StorageResult<usize> {
        self.inner.flush().await
    }

    /// Return the last `n` session records (newest-first).
    /// # Errors
    /// Returns [`StorageError`] if the buffer cannot be flushed or the records
    /// cannot be scanned.
    pub async fn recent(&self, n: usize) -> StorageResult<Vec<SessionRecord>> {
        // Flush first so buffered records are included.
        self.inner.flush().await?;
        let docs = self
            .inner
            .doc_store
            .scan_documents(&self.inner.session_id.to_string(), n)
            .await?;
        decode_records(docs)
    }

    /// Return all session records (oldest-first).
    /// # Errors
    /// Returns [`StorageError`] if the buffer cannot be flushed or the records
    /// cannot be scanned.
    pub async fn all(&self) -> StorageResult<Vec<SessionRecord>> {
        self.inner.flush().await?;
        // `!Send` on wasm (JS-backed stores); wrapped so this future stays
        // `Send` there too — wasm has one thread.
        let mut stream = SendWrapper::new(
            self.inner
                .doc_store
                .scan_all::<SessionRecord>(&self.inner.session_id.to_string())
                .await?,
        );
        let mut records = Vec::new();
        while let Some(record) = SendWrapper::new(stream.next()).await {
            records.push(record?);
        }
        Ok(records)
    }

    /// Scan from a given id (inclusive), returning up to `n` records.
    /// # Errors
    /// Returns [`StorageError`] if the buffer cannot be flushed or the records
    /// cannot be scanned.
    pub async fn scan_from(&self, from_id: &str, n: usize) -> StorageResult<Vec<SessionRecord>> {
        self.inner.flush().await?;
        let docs = self
            .inner
            .doc_store
            .scan_documents_from(&self.inner.session_id.to_string(), from_id, n)
            .await?;
        decode_records(docs)
    }

    /// Clear all session records (for testing).
    /// # Errors
    /// Returns [`StorageError`] if the buffer cannot be flushed or the
    /// records cannot be deleted.
    pub async fn clear(&self) -> StorageResult<u64> {
        self.inner.flush().await?;
        self.inner
            .doc_store
            .delete_all(&self.inner.session_id.to_string())
            .await
    }
}

/// Decode stored documents back into session records.
fn decode_records(docs: Vec<Document>) -> StorageResult<Vec<SessionRecord>> {
    docs.into_iter()
        .map(|d| serde_json::from_str::<SessionRecord>(&d.content))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| StorageError::Deserialization(e.to_string()))
}
