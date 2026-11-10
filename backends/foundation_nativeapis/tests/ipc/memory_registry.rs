//! `ipc::memory_registry` tests (moved from `src/ipc/memory_registry.rs`).

use std::time::Duration;

use foundation_nativeapis::ipc::memory_registry::MemoryRegistry;

#[test]
fn alloc_creates_new_region() {
    let mut registry = MemoryRegistry::new(foundation_core::valtron::Ctx::fixed(
        std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
    ));
    let region = registry.alloc(1024, None);
    assert!(region.is_some());
}

#[test]
fn alloc_with_free_runs_callback() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let mut registry = MemoryRegistry::new(foundation_core::valtron::Ctx::fixed(
        std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
    ));
    let freed = std::sync::Arc::new(AtomicBool::new(false));
    let freed_clone = freed.clone();

    let _region = registry.alloc_with_free(1024, None, move || {
        freed_clone.store(true, Ordering::SeqCst);
    });

    // The guard is held in the cache entry, so callback hasn't run yet
    assert!(!freed.load(Ordering::SeqCst));

    // After maintain (expiry), the guard should be dropped
    std::thread::sleep(Duration::from_secs(6));
    registry.maintain();

    assert!(freed.load(Ordering::SeqCst));
}
