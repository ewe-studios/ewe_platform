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
    if k == 0 {
        return Vec::new();
    }

    let mut heap: BinaryHeap<Reverse<HeapEntry>> = BinaryHeap::with_capacity(k + 1);

    for (id, vec) in entries {
        let score = metric.score(query, vec);
        if score.is_nan() {
            continue;
        }
        let entry = Reverse(HeapEntry {
            score: OrderedScore(score),
            id: id.to_string(),
        });

        if heap.len() < k {
            heap.push(entry);
        } else if let Some(min) = heap.peek() {
            if entry.0.score > min.0.score
                || (entry.0.score == min.0.score && entry.0.id < min.0.id)
            {
                heap.pop();
                heap.push(entry);
            }
        }
    }

    let results: Vec<VectorMatch> = heap
        .into_sorted_vec()
        .into_iter()
        .map(|Reverse(e)| VectorMatch {
            id: e.id,
            score: e.score.0,
        })
        .collect();
    results
}

/// Brute-force top-k over owned entries (for fetch-based backends).
#[must_use]
pub fn flat_top_k_owned(
    query: &[f32],
    entries: impl Iterator<Item = (String, Vec<f32>)>,
    k: usize,
    metric: DistanceMetric,
) -> Vec<VectorMatch> {
    if k == 0 {
        return Vec::new();
    }

    let mut heap: BinaryHeap<Reverse<HeapEntry>> = BinaryHeap::with_capacity(k + 1);

    for (id, vec) in entries {
        let score = metric.score(query, &vec);
        if score.is_nan() {
            continue;
        }
        let entry = Reverse(HeapEntry {
            score: OrderedScore(score),
            id,
        });

        if heap.len() < k {
            heap.push(entry);
        } else if let Some(min) = heap.peek() {
            if entry.0.score > min.0.score
                || (entry.0.score == min.0.score && entry.0.id < min.0.id)
            {
                heap.pop();
                heap.push(entry);
            }
        }
    }

    let results: Vec<VectorMatch> = heap
        .into_sorted_vec()
        .into_iter()
        .map(|Reverse(e)| VectorMatch {
            id: e.id,
            score: e.score.0,
        })
        .collect();
    results
}
