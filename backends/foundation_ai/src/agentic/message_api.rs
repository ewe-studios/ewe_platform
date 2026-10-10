//! `MessageApi` — the session's authoritative append-only audit log (F08).
//!
//! WHY: Every interaction must be durably recorded, replayable, and observable
//! in real time — without per-record disk latency stalling the agent loop.
//!
//! WHAT: `Arc<MessageInner>` over `DocumentStore` (F06), a write buffer +
//! flush task, and `&self` pub/sub for listeners. Semantic search is optional
//! (gated behind vector-store/embedding providers; the core append/flush/pub-sub
//! works without them).

use concurrent_queue::ConcurrentQueue;
pub use foundation_core::synca::mpp::Receiver;
use foundation_core::synca::mpp::TrackedBroadcaster;
use foundation_core::valtron::Stream;
use foundation_db::traits::DocumentStore;
use foundation_db::StorageResult;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    /// A batch was flushed to the `DocumentStore`.
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
// MessageInner

struct MessageInner<D> {
    session_id: SessionId,
    /// Shared so the session can hand the same store to other components
    /// (e.g. the memory coordinator's audit log) instead of a second copy.
    doc_store: Arc<D>,
    write_buffer: ConcurrentQueue<SessionRecord>,
    broadcaster: TrackedBroadcaster<MessageEvent>,
    /// Maximum buffered records before flush is triggered (default: 50).
    flush_threshold: usize,
    /// Pending flush flag — set when a flush is in progress.
    flush_pending: std::sync::atomic::AtomicBool,
    /// A record the store refused on the last flush. The next flush writes it
    /// first, so a failed flush neither loses the record nor reorders the log.
    retry: std::sync::Mutex<Option<SessionRecord>>,
}

impl<D: DocumentStore> MessageInner<D> {
    /// Flush all buffered records to the `DocumentStore`.
    ///
    /// On a store error the failing record is kept for the next flush (see
    /// `retry`), the records behind it stay buffered, and the in-progress flag
    /// is cleared — so a later flush can succeed instead of every flush after
    /// the first error silently returning `Ok(0)`.
    fn flush(&self) -> StorageResult<usize> {
        if self
            .flush_pending
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(0); // another flush in progress
        }
        let result = self.flush_buffered();
        self.flush_pending
            .store(false, std::sync::atomic::Ordering::SeqCst);
        result
    }

    fn flush_buffered(&self) -> StorageResult<usize> {
        let retry = self
            .retry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let mut count = 0;
        let mut pending = retry
            .into_iter()
            .chain(std::iter::from_fn(|| self.write_buffer.pop().ok()));
        let outcome = loop {
            let Some(record) = pending.next() else {
                break Ok(count);
            };
            if let Err(error) = self.write(&record) {
                *self
                    .retry
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(record);
                self.broadcaster.broadcast(MessageEvent::Error {
                    msg: format!("flushing session {}: {error}", self.session_id),
                });
                break Err(error);
            }
            count += 1;
        };
        if count > 0 {
            self.broadcaster.broadcast(MessageEvent::Flushed { count });
        }
        outcome
    }

    /// Write one record to the store.
    fn write(&self, record: &SessionRecord) -> StorageResult<()> {
        let key = self.session_id.to_string();
        // append_with_id for conversation records (stable id),
        // append_promotable for memory records (promotes columns).
        match record {
            SessionRecord::Conversation { message } => {
                self.doc_store
                    .append_with_id(&key, &message.id().to_string(), record.clone())?;
            }
            SessionRecord::WorkingMemory { .. }
            | SessionRecord::Observation { .. }
            | SessionRecord::Reflection { .. } => {
                self.doc_store.append_promotable(&key, record.clone())?;
            }
            _ => {
                self.doc_store.append(&key, record.clone())?;
            }
        }
        Ok(())
    }

    /// Records not yet written: the buffer plus a record held back by a
    /// failed flush.
    fn unflushed(&self) -> usize {
        let held = usize::from(
            self.retry
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some(),
        );
        self.write_buffer.len() + held
    }

    /// Check if the buffer has reached the flush threshold.
    fn should_flush(&self) -> bool {
        self.write_buffer.len() >= self.flush_threshold
    }
}

// ---------------------------------------------------------------------------
// MessageApi

/// The session's authoritative append-only message log. Cheap to clone
/// (`Arc`-shared) across valtron tasks.
///
/// # Example
/// ```ignore
/// let api = MessageApi::new(session_id, doc_store);
/// api.append(record);
/// let records = api.recent(10);
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
    /// Create a new `MessageApi` over the given `DocumentStore`.
    pub fn new(session_id: SessionId, doc_store: D) -> Self {
        Self::with_config(session_id, doc_store, 50, 256)
    }

    /// Create over a store that is shared with other components.
    pub fn from_shared(session_id: SessionId, doc_store: Arc<D>) -> Self {
        Self::shared_with_config(session_id, doc_store, 50, 256)
    }

    /// Create with explicit buffer and subscriber-channel capacity.
    pub fn with_config(
        session_id: SessionId,
        doc_store: D,
        flush_threshold: usize,
        subscriber_capacity: usize,
    ) -> Self {
        Self::shared_with_config(
            session_id,
            Arc::new(doc_store),
            flush_threshold,
            subscriber_capacity,
        )
    }

    /// [`with_config`](Self::with_config) over a shared store.
    pub fn shared_with_config(
        session_id: SessionId,
        doc_store: Arc<D>,
        flush_threshold: usize,
        subscriber_capacity: usize,
    ) -> Self {
        Self {
            inner: Arc::new(MessageInner {
                session_id,
                doc_store,
                write_buffer: ConcurrentQueue::unbounded(),
                broadcaster: TrackedBroadcaster::new(subscriber_capacity),
                flush_threshold,
                flush_pending: std::sync::atomic::AtomicBool::new(false),
                retry: std::sync::Mutex::new(None),
            }),
        }
    }

    /// Subscribe to message events (appends, flushes, errors).
    #[must_use]
    pub fn subscribe(&self) -> Receiver<MessageEvent> {
        self.inner.broadcaster.subscribe()
    }
}

impl<D: DocumentStore> MessageApi<D> {
    /// Append a record to the session log. Returns the record's scru128 id.
    ///
    /// The record is buffered (no disk I/O); a background flush task drains
    /// the buffer periodically or when it reaches `flush_threshold`.
    #[must_use]
    /// # Errors
    /// Returns [`StorageError`] if the record cannot be appended.
    pub fn append(&self, record: SessionRecord) -> String {
        let id = match &record {
            SessionRecord::Conversation { message } => message.id().to_string(),
            _ => foundation_compact::ids::new_scru128_string(),
        };
        let variant = MessageEvent::variant_name(&record);
        self.inner
            .write_buffer
            .push(record)
            .expect("unbounded queue never fails");

        self.inner.broadcaster.broadcast(MessageEvent::Appended {
            id: id.clone(),
            variant,
        });

        // Trigger flush if buffer is full.
        if self.inner.should_flush() {
            // `append` only buffers; a failed threshold flush keeps every
            // record (see `MessageInner::flush`) and the next `flush()` —
            // `AgentSession::end()` at the latest — reports the error.
            if let Err(error) = self.inner.flush() {
                tracing::warn!(
                    session = %self.inner.session_id,
                    %error,
                    "threshold flush failed; records stay buffered for the next flush"
                );
            }
        }

        id
    }

    /// Flush all buffered records to the `DocumentStore` immediately.
    /// # Errors
    /// Returns [`StorageError`] if the records cannot be scanned.
    pub fn flush(&self) -> StorageResult<usize> {
        self.inner.flush()
    }

    /// Records appended but not yet written to the store (including one held
    /// back by a failed flush).
    #[must_use]
    pub fn unflushed(&self) -> usize {
        self.inner.unflushed()
    }

    /// Return the last `n` session records (newest-first).
    /// # Errors
    /// Returns [`StorageError`] if the records cannot be scanned.
    pub fn recent(&self, n: usize) -> StorageResult<Vec<SessionRecord>> {
        // Flush first so buffered records are included.
        self.inner.flush()?;
        let docs = self
            .inner
            .doc_store
            .scan_documents(&self.inner.session_id.to_string(), n)?;
        docs.into_iter()
            .map(|d| serde_json::from_str::<SessionRecord>(&d.content))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| foundation_db::StorageError::Deserialization(e.to_string()))
    }

    /// Return all session records (oldest-first) as an iterator.
    /// # Errors
    /// Returns [`StorageError`] if the records cannot be scanned.
    pub fn all(&self) -> StorageResult<Vec<SessionRecord>> {
        self.inner.flush()?;
        let stream = self
            .inner
            .doc_store
            .scan_all::<SessionRecord>(&self.inner.session_id.to_string())?;
        let mut records = Vec::new();
        for item in stream {
            if let Stream::Next(result) = item {
                records.push(result?);
            }
        }
        Ok(records)
    }

    /// Scan from a given id (inclusive), returning up to `n` records.
    /// # Errors
    /// Returns [`StorageError`] if the records cannot be counted.
    pub fn scan_from(&self, from_id: &str, n: usize) -> StorageResult<Vec<SessionRecord>> {
        self.inner.flush()?;
        let docs = self.inner.doc_store.scan_documents_from(
            &self.inner.session_id.to_string(),
            from_id,
            n,
        )?;
        docs.into_iter()
            .map(|d| serde_json::from_str::<SessionRecord>(&d.content))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| foundation_db::StorageError::Deserialization(e.to_string()))
    }

    /// Clear all session records (for testing).
    /// # Errors
    /// Returns [`StorageError`] if the buffer cannot be flushed.
    pub fn clear(&self) -> StorageResult<u64> {
        self.inner.flush()?;
        self.inner
            .doc_store
            .delete_all(&self.inner.session_id.to_string())
    }
}

// ---------------------------------------------------------------------------
