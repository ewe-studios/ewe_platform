/// Pooled allocator for `MemoryRegion`.

use std::collections::BTreeMap;
use std::time::Instant;

use super::platform::MemoryRegion;

/// Entry in the memory region cache.
struct MemoryRegionEntry {
    region: MemoryRegion,
    last_alloc: Instant,
    tag: Option<String>,
}

/// Pooled allocator for shared memory regions.
#[derive(Default)]
pub struct MemoryRegistry {
    inner: BTreeMap<usize, Vec<MemoryRegionEntry>>,
}

impl MemoryRegistry {
    const EXPIRY_SECS: u64 = 5;

    /// Allocate a region of at least `min_size` bytes.
    pub fn alloc(&mut self, min_size: usize, tag: Option<&str>) -> Option<MemoryRegion> {
        self.maintain();

        let upper = min_size.saturating_mul(2).max(min_size + 1);

        // Collect ranges first to avoid borrow checker issues
        let ranges: Vec<usize> = self.inner.keys().copied()
            .filter(|&k| k >= min_size && k < upper)
            .collect();

        for key in ranges {
            if let Some(entries) = self.inner.get_mut(&key) {
                for entry in entries.iter_mut() {
                    if entry.region.ref_count() == 1 {
                        if let Some(t) = tag {
                            if entry.tag.as_deref() == Some(t) {
                                entry.last_alloc = Instant::now();
                                return entry.region.try_clone().ok();
                            }
                        } else {
                            entry.last_alloc = Instant::now();
                            return entry.region.try_clone().ok();
                        }
                    }
                }
            }
        }

        let region = MemoryRegion::new(min_size)?;
        self.inner
            .entry(min_size)
            .or_default()
            .push(MemoryRegionEntry {
                region: region.try_clone().ok()?,
                last_alloc: Instant::now(),
                tag: tag.map(String::from),
            });

        Some(region)
    }

    fn maintain(&mut self) {
        let now = Instant::now();
        let expiry = std::time::Duration::from_secs(Self::EXPIRY_SECS);

        self.inner.retain(|_, entries| {
            entries.retain(|entry| now.duration_since(entry.last_alloc) < expiry);
            !entries.is_empty()
        });
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
        let region = region.unwrap();
        assert!(region.buffer_size() >= 1024);
    }
}
