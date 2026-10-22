//! Brute-force flat-scan top-k with a bounded min-heap.

use crate::metric::{DistanceMetric, OrderedScore};
use crate::store::VectorMatch;
use std::collections::BinaryHeap;
use std::cmp::Reverse;

struct HeapEntry {
    score: OrderedScore,
    id: String,
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.id == other.id
    }
}

impl Eq for HeapEntry {}

impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.cmp(&other.score)
            .then_with(|| other.id.cmp(&self.id))
    }
}

/// A bounded top-k accumulator: feed it candidates one at a time, then take
/// the best `k`. O(k) memory however many candidates are pushed, so a caller
/// can rank rows as they stream in instead of collecting them first.
///
/// Ties are broken by id ascending for deterministic results. NaN scores are
/// never selected.
pub struct TopK<'q> {
    query: &'q [f32],
    k: usize,
    metric: DistanceMetric,
    heap: BinaryHeap<Reverse<HeapEntry>>,
}

impl<'q> TopK<'q> {
    /// An empty accumulator for the `k` best matches of `query`.
    #[must_use]
    pub fn new(query: &'q [f32], k: usize, metric: DistanceMetric) -> Self {
        Self {
            query,
            k,
            metric,
            heap: BinaryHeap::with_capacity(k.saturating_add(1)),
        }
    }

    /// Score one candidate and keep it if it is among the best `k` so far.
    /// The id is copied only when the candidate is kept.
    pub fn push(&mut self, id: &str, vec: &[f32]) {
        if self.k == 0 {
            return;
        }
        let score = self.metric.score(self.query, vec);
        if score.is_nan() {
            return;
        }
        let score = OrderedScore(score);
        if self.heap.len() < self.k {
            self.heap.push(Reverse(HeapEntry { score, id: id.to_string() }));
        } else if let Some(min) = self.heap.peek() {
            if score > min.0.score || (score == min.0.score && id < min.0.id.as_str()) {
                self.heap.pop();
                self.heap.push(Reverse(HeapEntry { score, id: id.to_string() }));
            }
        }
    }

    /// The kept matches, best first.
    #[must_use]
    pub fn finish(self) -> Vec<VectorMatch> {
        self.heap
            .into_sorted_vec()
            .into_iter()
            .map(|Reverse(e)| VectorMatch {
                id: e.id,
                score: e.score.0,
            })
            .collect()
    }
}

/// Brute-force top-k over borrowed entries. O(n*d) time, O(k) memory.
///
/// Uses a bounded min-heap of size k. Ties are broken by id ascending
/// for deterministic results. NaN scores are never selected (`OrderedScore`
/// sorts NaN as least).
#[must_use]
pub fn flat_top_k<'a>(
    query: &[f32],
    entries: impl Iterator<Item = (&'a str, &'a [f32])>,
    k: usize,
    metric: DistanceMetric,
) -> Vec<VectorMatch> {
    let mut top = TopK::new(query, k, metric);
    for (id, vec) in entries {
        top.push(id, vec);
    }
    top.finish()
}

/// Brute-force top-k over owned entries (for fetch-based backends).
#[must_use]
pub fn flat_top_k_owned(
    query: &[f32],
    entries: impl Iterator<Item = (String, Vec<f32>)>,
    k: usize,
    metric: DistanceMetric,
) -> Vec<VectorMatch> {
    let mut top = TopK::new(query, k, metric);
    for (id, vec) in entries {
        top.push(&id, &vec);
    }
    top.finish()
}
