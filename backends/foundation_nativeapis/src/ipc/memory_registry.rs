/// Pooled allocator for `MemoryRegion`.
///
/// Instead of creating a new shared memory object on every send,
/// `MemoryRegistry` keeps a cache of recently-used regions and reuses them
/// when possible.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::MemoryRegion;

/// Entry in the memory region cache.
struct MemoryRegionEntry {
    region: MemoryRegion,
    last_alloc: Instant,
    tag: Option<String>,
    guard: Guard,
}

/// Guard that runs a callback when dropped.
struct Guard {
    free: Option<Box<dyn FnOnce()>>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(free) = self.free.take() {
            free();
        }
    }
}

/// Pooled allocator for shared memory regions.
#[derive(Default)]
pub struct MemoryRegistry {
    inner: BTreeMap<usize, Vec<MemoryRegionEntry>>,
}

impl MemoryRegistry {
    /// Allocate a region of at least `min_size` bytes.
    pub fn alloc(&mut self, min_size: usize, tag: Option<&str>) -> Option<MemoryRegion> {
        self.alloc_inner(min_size, tag, None)
    }

    /// Allocate a region with a callback that runs when the cached entry is evicted.
    pub fn alloc_with_free(
        &mut self,
        min_size: usize,
        tag: Option<&str>,
        free: impl FnOnce() + 'static,
    ) -> Option<MemoryRegion> {
        let free: Box<dyn FnOnce()> = Box::new(free);
        self.alloc_inner(min_size, tag, Some(free))
    }

    fn alloc_inner(
        &mut self,
        min_size: usize,
        tag: Option<&str>,
        free: Option<Box<dyn FnOnce()>>,
    ) -> Option<MemoryRegion> {
        let now = Instant::now();

        // Search for reusable entries in range [min_size .. min_size * 2)
        for (_, rs) in self.inner.range_mut(min_size..min_size.saturating_mul(2).max(min_size + 1)) {
            for r in rs {
                if r.region.ref_count() == 1 && tag == r.tag.as_deref() {
                    r.last_alloc = now;

                    let region = r.region.clone().ok()?;
                    r.guard = Guard { free };

                    self.maintain_inner(now);

                    return Some(region);
                }
            }
        }

        self.maintain_inner(now);

        let r = MemoryRegion::new(min_size)?;
        self.inner
            .entry(min_size)
            .or_default()
            .push(MemoryRegionEntry {
                region: r.clone().ok()?,
                last_alloc: now,
                tag: tag.map(ToOwned::to_owned),
                guard: Guard { free },
            });
        Some(r)
    }

    fn maintain_inner(&mut self, now: Instant) {
        self.inner.retain(|_, rs| {
            rs.retain_mut(|r| {
                let should_retain = (now - r.last_alloc) < Duration::from_secs(5);

                if r.region.ref_count() == 1 {
                    r.guard = Guard { free: None };
                }

                should_retain
            });
            !rs.is_empty()
        });
    }

    pub fn maintain(&mut self) {
        self.maintain_inner(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_creates_new_region() {
        let mut registry = MemoryRegistry::default();
        let region = registry.alloc(1024, None);
        assert!(region.is_some());
    }

    #[test]
    fn alloc_with_free_runs_callback() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let mut registry = MemoryRegistry::default();
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
}
