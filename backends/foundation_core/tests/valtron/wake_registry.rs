//! `WakeRegistry`, moved out of `src/valtron/wake.rs` (tests live in `tests/`).

use foundation_core::valtron::WakeRegistry;
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
