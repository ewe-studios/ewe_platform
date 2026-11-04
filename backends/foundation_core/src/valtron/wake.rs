//! Wake side-channel for valtron stream processing (Feature 66 → 02-wakeable-stream).
//!
//! [`WakeableStream`] is an **opt-in** trait (supertrait [`StreamIterator`]) whose
//! `wake(Fn() -> ())` registers a one-shot readiness callback on the backing queue.
//! The producer side (`NotifyQueue::push` / `close`) fires every registered callback
//! and clears the list; the consumer side (the `stream_future.rs` bridges, wired in
//! feature 03) re-polls after being woken. The wake signal carries no payload, so the
//! producer fires on every `push` and on `close` — the woken bridge re-polls and
//! re-classifies. This is the `Waker::wake()` contract.
//!
//! ## Why opt-in (no blanket)
//!
//! A blanket `impl<M: Iterator<Item = Stream<D,P>>> WakeableStream for M` (default
//! `NotSupported`) would overlap the waking terminal's `Successful` impl — E0119, no
//! stable specialization. Dropping the `StreamIterator` blanket instead is ~10× worse.
//! So: keep the blanket, make `WakeableStream` opt-in, implement it explicitly on the
//! handful of types that matter.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::compati::Mutex;
use crate::valtron::streams::StreamIterator;

/// Result of attempting to register a wake callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeResult {
    /// The callback was registered and will fire exactly once on the next readiness
    /// edge (a `push` or a transition to closed) of the backing queue.
    Successful,
    /// The stream cannot deliver a readiness signal; the caller must self-wake.
    NotSupported,
}

/// One-shot readiness callback. Fired exactly once (and the list cleared) when the
/// backing queue has/will have an item, or closes.
#[cfg(feature = "multi")]
pub type WakeCallback = Box<dyn Fn() + Send + 'static>;
#[cfg(not(feature = "multi"))]
pub type WakeCallback = Box<dyn Fn() + 'static>;

/// Opt-in readiness side-channel for [`StreamIterator`]s.
///
/// The default implementation returns [`WakeResult::NotSupported`]; only types that
/// can actually deliver a wake signal override it. The `StreamIterator` blanket impl
/// is intentionally left in place, and this trait has **no** blanket impl of its own.
pub trait WakeableStream: StreamIterator {
    /// Register `cb` to be called once when the stream becomes ready (an item is
    /// available, or the queue closes). Returns [`WakeResult::Successful`] if the
    /// stream can deliver the signal, [`WakeResult::NotSupported`] otherwise.
    fn wake(&mut self, cb: WakeCallback) -> WakeResult {
        let _ = cb; // default: cannot deliver the signal
        WakeResult::NotSupported
    }

    /// Idempotently announce that an async consumer will pull this stream,
    /// before the first pull, so a blocking terminal answers an empty pull with
    /// `Wait` instead of blocking the polling thread. Cheap and safe to call on
    /// every poll; it must not register a wake callback (that is what
    /// [`Self::wake`] is for).
    ///
    /// Defaults to a no-op: a stream that cannot signal readiness has nothing
    /// to announce. A wrapper that forwards [`Self::wake`] to an inner waking
    /// terminal must also forward `announce` to it, or the inner first empty
    /// pull blocks again.
    fn announce(&mut self) {}
}

/// Fire-all-then-clear registry.
///
/// `fire()` takes the callback vec under the lock, releases the lock, then calls each
/// callback. User code is never run under the lock (a callback that re-enters the
/// registry must not deadlock).
pub struct WakeRegistry {
    callbacks: Mutex<Vec<WakeCallback>>,
    /// Number of callbacks currently held. Mirrors `callbacks.len()` but is an
    /// `AtomicUsize` so a fast path can gate a `fire()` without taking the lock
    /// (see [`Self::has_waiters`]). Maintained under the same lock as the vec in
    /// `register`, `replace` and `fire`, so it never disagrees with the list.
    pending: AtomicUsize,
}

impl WakeRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            callbacks: Mutex::new(Vec::new()),
            pending: AtomicUsize::new(0),
        }
    }

    /// Registers a one-shot readiness callback.
    pub fn register(&self, cb: WakeCallback) {
        let mut callbacks = self.callbacks.lock().unwrap();
        callbacks.push(cb);
        self.pending.store(callbacks.len(), Ordering::SeqCst);
    }

    /// Replaces every registered callback with a single one. A producer parking
    /// on a full queue re-registers on each retry; keeping only the latest avoids
    /// unbounded growth of stale callbacks while the queue stays full.
    pub fn replace(&self, cb: WakeCallback) {
        let mut callbacks = self.callbacks.lock().unwrap();
        callbacks.clear();
        callbacks.push(cb);
        self.pending.store(callbacks.len(), Ordering::SeqCst);
    }

    /// True when at least one callback is registered. A `fire()` can be skipped
    /// when false, since there is nothing to wake.
    pub fn has_waiters(&self) -> bool {
        self.pending.load(Ordering::SeqCst) > 0
    }

    /// Number of callbacks currently registered (tests use it to pin no growth).
    pub fn pending_len(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }

    /// Fires every registered callback exactly once, then clears the list.
    pub fn fire(&self) {
        let callbacks = std::mem::take(&mut *self.callbacks.lock().unwrap());
        self.pending.store(0, Ordering::SeqCst);
        for cb in callbacks {
            cb();
        }
    }
}

impl Default for WakeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for WakeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WakeRegistry")
            .field("pending", &self.pending_len())
            .finish()
    }
}
