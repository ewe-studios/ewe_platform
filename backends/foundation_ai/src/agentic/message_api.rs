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
        }
    }
}

// ---------------------------------------------------------------------------
// Subscriber — &self pub/sub (CRIT-02 fix)

/// A `&self`-safe broadcaster built on `Arc<Mutex<Vec<ConcurrentQueue>>>`.
/// Each subscriber gets its own `Receiver<MessageEvent>` backed by a bounded
/// `ConcurrentQueue`; `try_push` never blocks (drops on back-pressure).
struct Subscribers {
    queues: std::sync::Mutex<Vec<Arc<ConcurrentQueue<MessageEvent>>>>,
    capacity: usize,
}

impl Subscribers {
    fn new(capacity: usize) -> Self {
        Self {
            queues: std::sync::Mutex::new(Vec::new()),
            capacity,
        }
    }

    fn subscribe(&self) -> Receiver<MessageEvent> {
        let chan = Arc::new(ConcurrentQueue::bounded(self.capacity));
        self.queues.lock().unwrap().push(chan.clone());
        Receiver::new(chan)
    }

    fn broadcast(&self, event: MessageEvent) {
        let mut stale = Vec::new();
        let mut queues = self.queues.lock().unwrap();
        for (i, q) in queues.iter().enumerate() {
            // Force-push (evicts oldest if full — subscribers that can't keep up
            // get partial history, which is acceptable for observability).
            if q.force_push(event.clone()).is_err() {
                stale.push(i);
            }
        }
        // Remove stale subscribers (all were force-pushed so this is dead code,
        // but keep the cleanup path for future bounded-push changes).
        for i in stale.into_iter().rev() {
            queues.remove(i);
        }
    }
}

// A bounded channel receiver for MessageEvent.
pub struct Receiver<T> {
    chan: Arc<ConcurrentQueue<T>>,
}

impl<T> Receiver<T> {
    fn new(chan: Arc<ConcurrentQueue<T>>) -> Self {
        Self { chan }
    }

    /// Try to receive the next message (non-blocking).
    pub fn try_recv(&self) -> Result<T, concurrent_queue::PopError> {
        self.chan.pop()
    }

    /// Iterate all currently buffered messages (non-blocking drain).
    pub fn try_iter(&self) -> concurrent_queue::TryIter<'_, T> {
        self.chan.try_iter()
    }
}

// ---------------------------------------------------------------------------
// MessageInner

struct MessageInner<D> {
    session_id: SessionId,
    doc_store: D,
    write_buffer: ConcurrentQueue<SessionRecord>,
    subscribers: Arc<Subscribers>,
    /// Maximum buffered records before flush is triggered (default: 50).
    flush_threshold: usize,
    /// Pending flush flag — set when a flush is in progress.
    flush_pending: std::sync::atomic::AtomicBool,
}

impl<D: DocumentStore> MessageInner<D> {
    /// Flush all buffered records to the `DocumentStore`.
    fn flush(&self) -> StorageResult<usize> {
        if self
            .flush_pending
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(0); // another flush in progress
        }
        let mut count = 0;
        loop {
            match self.write_buffer.pop() {
                Ok(record) => {
                    let id = match &record {
                        SessionRecord::Conversation { message } => message.id().to_string(),
                        _ => foundation_compact::ids::new_scru128_string(),
                    };
                    // Use append_with_id for conversation records (stable id),
                    // append_promotable for memory records (promotes columns).
                    match &record {
                        SessionRecord::Conversation { .. } => {
                            self.doc_store.append_with_id(
                                &self.session_id.to_string(),
                                &id,
                                record.clone(),
                            )?;
                        }
                        SessionRecord::WorkingMemory { .. }
                        | SessionRecord::Observation { .. }
                        | SessionRecord::Reflection { .. } => {
                            self.doc_store
                                .append_promotable(&self.session_id.to_string(), record.clone())?;
                        }
                        _ => {
                            self.doc_store
                                .append(&self.session_id.to_string(), record.clone())?;
                        }
                    }
                    count += 1;
                }
                Err(_) => break, // queue empty
            }
        }
        self.flush_pending
            .store(false, std::sync::atomic::Ordering::SeqCst);
        if count > 0 {
            self.subscribers.broadcast(MessageEvent::Flushed { count });
        }
        Ok(count)
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

    /// Create with explicit buffer and subscriber-channel capacity.
    pub fn with_config(
        session_id: SessionId,
        doc_store: D,
        flush_threshold: usize,
        subscriber_capacity: usize,
    ) -> Self {
        Self {
            inner: Arc::new(MessageInner {
                session_id,
                doc_store,
                write_buffer: ConcurrentQueue::unbounded(),
                subscribers: Arc::new(Subscribers::new(subscriber_capacity)),
                flush_threshold,
                flush_pending: std::sync::atomic::AtomicBool::new(false),
            }),
        }
    }

    /// Subscribe to message events (appends, flushes, errors).
    #[must_use]
    pub fn subscribe(&self) -> Receiver<MessageEvent> {
        self.inner.subscribers.subscribe()
    }
}

impl<D: DocumentStore> MessageApi<D> {
    /// Append a record to the session log. Returns the record's scru128 id.
    ///
    /// The record is buffered (no disk I/O); a background flush task drains
    /// the buffer periodically or when it reaches `flush_threshold`.
    #[must_use]
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

        self.inner.subscribers.broadcast(MessageEvent::Appended {
            id: id.clone(),
            variant,
        });

        // Trigger flush if buffer is full.
        if self.inner.should_flush() {
            let _ = self.inner.flush();
        }

        id
    }

    /// Flush all buffered records to the `DocumentStore` immediately.
    pub fn flush(&self) -> StorageResult<usize> {
        self.inner.flush()
    }

    /// Return the last `n` session records (newest-first).
    pub fn recent(&self, n: usize) -> StorageResult<Vec<SessionRecord>> {
        // Flush first so buffered records are included.
        let _ = self.inner.flush();
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
    pub fn all(&self) -> StorageResult<Vec<SessionRecord>> {
        let _ = self.inner.flush();
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
    pub fn scan_from(&self, from_id: &str, n: usize) -> StorageResult<Vec<SessionRecord>> {
        let _ = self.inner.flush();
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
    pub fn clear(&self) -> StorageResult<u64> {
        let _ = self.inner.flush();
        self.inner
            .doc_store
            .delete_all(&self.inner.session_id.to_string())
    }
}

// ---------------------------------------------------------------------------
