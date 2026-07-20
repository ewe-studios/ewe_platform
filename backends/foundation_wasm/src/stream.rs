//! ConcurrentQueue-based stream registry for wasm runtime (F28 wasm side).
//!
//! WHY: WASM code needs to stream responses back to the host (browser, Tauri,
//! Deno). This provides a lock-free, wasm32-safe registry of
//! `ConcurrentQueue`-backed streams that the WASM side writes to and the
//! JS host drains.
//!
//! WHAT: `StreamRegistry` — maps `StreamId → Arc<ConcurrentQueue<StreamChunk>>`.
//! WASM calls `create_stream()` to get an ID, `send()` to push chunks, and
//! `close()` to signal end-of-stream.
//!
//! HOW: Uses `concurrent_queue::ConcurrentQueue` (workspace dep, supports
//! `no_std` with `default-features = false`). Registry is `Mutex<BTreeMap>` on
//! native, plain `BTreeMap` on wasm32. Queues are `Arc`-wrapped for shared
//! ownership between the registry and the host drainer.

use alloc::collections::btree_map::BTreeMap;
use alloc::sync::Arc;

use foundation_nostd::comp::basic::Mutex;

// ── Stream types ───────────────────────────────────────────────────────

/// Unique stream identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamId(pub u64);

/// A chunk in a wasm→host stream.
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub data: alloc::vec::Vec<u8>,
    pub sequence: u64,
}

impl StreamChunk {
    #[must_use]
    pub fn new(data: alloc::vec::Vec<u8>, sequence: u64) -> Self {
        Self { data, sequence }
    }
}

/// Type alias for the concurrent queue used in streams.
pub type StreamQueue = Arc<concurrent_queue::ConcurrentQueue<StreamChunk>>;

// ── StreamRegistry ─────────────────────────────────────────────────────

/// Registry of concurrent streams from WASM → host.
///
/// The WASM side creates a stream, pushes chunks via `send()`, and calls
/// `close()` when done. The host side (JS) drains the queue via the
/// returned `Arc<ConcurrentQueue<StreamChunk>>`.
///
/// On native: `Mutex<BTreeMap<...>>` for thread safety.
/// On wasm32: plain `BTreeMap` (single-threaded).
pub struct StreamRegistry {
    next_id: Mutex<u64>,
    streams: Mutex<BTreeMap<StreamId, StreamQueue>>,
}

impl StreamRegistry {
    /// Create an empty stream registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next_id: Mutex::new(1),
            streams: Mutex::new(BTreeMap::new()),
        }
    }

    /// Create a new stream and return its ID + queue handle.
    ///
    /// Takes `&self` — the internal `Mutex`es provide interior mutability.
    pub fn create(&self) -> (StreamId, StreamQueue) {
        let mut id_guard = self
            .next_id
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        let id = StreamId(*id_guard);
        *id_guard = id_guard.wrapping_add(1);
        drop(id_guard);

        let queue = Arc::new(concurrent_queue::ConcurrentQueue::unbounded());
        self.streams
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .insert(id, Arc::clone(&queue));
        (id, queue)
    }

    /// Push a chunk onto a stream.
    ///
    /// Returns `false` if the stream doesn't exist or the queue is closed.
    ///
    /// # Errors
    ///
    /// This function cannot error. It returns `false` for non-existent/closed
    /// streams rather than panicking.
    pub fn send(&self, id: StreamId, chunk: StreamChunk) -> bool {
        let map = self
            .streams
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        match map.get(&id) {
            Some(queue) => queue.push(chunk).is_ok(),
            None => false,
        }
    }

    /// Close a stream. Removes it from the registry and closes the queue.
    /// Future `send()` calls for this ID will return `false`.
    ///
    /// Returns `false` if the stream was already closed or doesn't exist.
    pub fn close(&self, id: StreamId) -> bool {
        let mut map = self
            .streams
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        if let Some(queue) = map.remove(&id) {
            queue.close();
            true
        } else {
            false
        }
    }
}

impl Default for StreamRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── WASM exports (F28) ──────────────────────────────────────────────────

#[cfg(feature = "web")]
mod wasm_ffi {
    use alloc::vec::Vec;
    use foundation_nostd::comp::basic::Mutex;
    use super::{StreamChunk, StreamId, StreamRegistry};

    static STREAM_REGISTRY: Mutex<StreamRegistry> = Mutex::new(StreamRegistry::new());

    /// Create a new stream. Returns the stream ID as u64.
    #[no_mangle]
    pub extern "C" fn stream_create() -> u64 {
        let (id, _queue) = STREAM_REGISTRY
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .create();
        id.0
    }

    /// Push a chunk onto a stream. `data_ptr`/`data_len` point to the chunk bytes.
    /// Returns 1 on success, 0 on failure (stream not found or queue closed).
    #[no_mangle]
    pub extern "C" fn stream_send(stream_id: u64, data_ptr: *const u8, data_len: u32, seq: u64) -> u32 {
        let data = unsafe { alloc::slice::from_raw_parts(data_ptr, data_len as usize) };
        let chunk = StreamChunk::new(data.to_vec(), seq);
        u32::from(STREAM_REGISTRY
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .send(StreamId(stream_id), chunk))
    }

    /// Close and deregister a stream. Returns 1 on success, 0 if already closed.
    #[no_mangle]
    pub extern "C" fn stream_close(stream_id: u64) -> u32 {
        u32::from(STREAM_REGISTRY
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .close(StreamId(stream_id)))
    }
}
