//! Work queue for reactive nodes. Lower heights run first.

use alloc::collections::{BTreeMap, VecDeque};

pub(crate) struct HeightQueue<K> {
    buckets: BTreeMap<usize, VecDeque<K>>,
}

impl<K> Default for HeightQueue<K> {
    fn default() -> Self {
        Self {
            buckets: BTreeMap::new(),
        }
    }
}

impl<K> HeightQueue<K> {
    pub(crate) fn push(&mut self, height: usize, id: K) {
        self.buckets.entry(height).or_default().push_back(id);
    }

    #[cfg(all(feature = "thread_safe", not(target_family = "wasm")))]
    pub(crate) fn pop_first(&mut self) -> Option<(usize, K)> {
        self.pop_at_or_after(0)
    }

    /// Keep lower work for a later pass. Preserve insertion order at the selected height.
    pub(crate) fn pop_at_or_after(&mut self, minimum: usize) -> Option<(usize, K)> {
        let (&height, bucket) = self.buckets.range_mut(minimum..).next()?;
        let id = bucket.pop_front().expect("A queued bucket contains work");
        if bucket.is_empty() {
            self.buckets.remove(&height);
        }
        Some((height, id))
    }
}
