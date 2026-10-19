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
}

/// Fire-all-then-clear registry.
///
/// `fire()` takes the callback vec under the lock, releases the lock, then calls each
/// callback. User code is never run under the lock (a callback that re-enters the
/// registry must not deadlock).
pub struct WakeRegistry {
    callbacks: Mutex<Vec<WakeCallback>>,
}

impl WakeRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            callbacks: Mutex::new(Vec::new()),
        }
    }

    /// Registers a one-shot readiness callback.
    pub fn register(&self, cb: WakeCallback) {
        self.callbacks.lock().unwrap().push(cb);
    }

    /// Fires every registered callback exactly once, then clears the list.
    pub fn fire(&self) {
        let callbacks = std::mem::take(&mut *self.callbacks.lock().unwrap());
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
            .field("pending", &self.callbacks.lock().unwrap().len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Each registered callback fires exactly once per `fire()`; the list is then
    /// cleared, so a second `fire()` is a no-op (no double-fire).
    #[test]
    fn wake_registry_fires_each_callback_once_then_clears() {
        let registry = WakeRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let c1 = Arc::clone(&counter);
        registry.register(Box::new(move || {
            c1.fetch_add(1, Ordering::SeqCst);
        }));
        let c2 = Arc::clone(&counter);
        registry.register(Box::new(move || {
            c2.fetch_add(1, Ordering::SeqCst);
        }));

        registry.fire();
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        // The vec was drained — nothing further to fire.
        registry.fire();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
