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

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<(String, Vec<f32>)> {
        vec![
            ("a".into(), vec![1.0, 0.0]),
            ("b".into(), vec![0.7, 0.7]),
            ("c".into(), vec![0.0, 1.0]),
            ("d".into(), vec![-1.0, 0.0]),
        ]
    }

    #[test]
    fn top_k_cosine() {
        let data = entries();
        let refs: Vec<(&str, &[f32])> = data.iter().map(|(id, v)| (id.as_str(), v.as_slice())).collect();
        let query = [1.0, 0.0];
        let results = flat_top_k(&query, refs.into_iter(), 2, DistanceMetric::Cosine);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "a");
        assert_eq!(results[1].id, "b");
    }

    #[test]
    fn top_k_l2() {
        let data = entries();
        let refs: Vec<(&str, &[f32])> = data.iter().map(|(id, v)| (id.as_str(), v.as_slice())).collect();
        let query = [0.0, 0.0];
        let results = flat_top_k(&query, refs.into_iter(), 1, DistanceMetric::L2);
        assert_eq!(results.len(), 1);
        // [0.7, 0.7] has squared L2 = 0.98, [1,0] and [0,1] have 1.0
        // neg_squared_l2: -0.98 > -1.0 so "b" is closest
        assert_eq!(results[0].id, "b");
    }

    #[test]
    fn k_zero_returns_empty() {
        let data = entries();
        let refs: Vec<(&str, &[f32])> = data.iter().map(|(id, v)| (id.as_str(), v.as_slice())).collect();
        let results = flat_top_k(&[1.0, 0.0], refs.into_iter(), 0, DistanceMetric::Cosine);
        assert!(results.is_empty());
    }

    #[test]
    fn k_greater_than_n() {
        let data = entries();
        let refs: Vec<(&str, &[f32])> = data.iter().map(|(id, v)| (id.as_str(), v.as_slice())).collect();
        let results = flat_top_k(&[1.0, 0.0], refs.into_iter(), 100, DistanceMetric::Cosine);
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn empty_entries() {
        let results = flat_top_k(&[1.0], std::iter::empty(), 5, DistanceMetric::Cosine);
        assert!(results.is_empty());
    }

    #[test]
    fn nan_scores_excluded() {
        let data: Vec<(&str, &[f32])> = vec![
            ("zero", &[0.0, 0.0]),
            ("good", &[1.0, 0.0]),
        ];
        let query = [1.0, 0.0];
        // cosine with [0,0] returns NEG_INFINITY (not NaN due to our guard), but let's test directly
        let results = flat_top_k(&query, data.into_iter(), 2, DistanceMetric::Cosine);
        // Both should appear — zero vector gets NEG_INFINITY score which is valid (not NaN)
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "good");
    }

    #[test]
    fn tie_breaking_by_id() {
        let data: Vec<(&str, &[f32])> = vec![
            ("b", &[1.0, 0.0]),
            ("a", &[1.0, 0.0]),
        ];
        let query = [1.0, 0.0];
        let results = flat_top_k(&query, data.into_iter(), 2, DistanceMetric::Cosine);
        assert_eq!(results[0].id, "a");
        assert_eq!(results[1].id, "b");
    }

    #[test]
    fn flat_top_k_owned_matches_borrowed() {
        let data = entries();
        let refs: Vec<(&str, &[f32])> = data.iter().map(|(id, v)| (id.as_str(), v.as_slice())).collect();
        let query = [1.0, 0.0];
        let borrowed = flat_top_k(&query, refs.into_iter(), 3, DistanceMetric::Cosine);
        let owned = flat_top_k_owned(&query, data.into_iter(), 3, DistanceMetric::Cosine);
        assert_eq!(borrowed.len(), owned.len());
        for (b, o) in borrowed.iter().zip(owned.iter()) {
            assert_eq!(b.id, o.id);
            assert!((b.score - o.score).abs() < 1e-6);
        }
    }

    #[test]
    fn results_sorted_descending() {
        let data = entries();
        let refs: Vec<(&str, &[f32])> = data.iter().map(|(id, v)| (id.as_str(), v.as_slice())).collect();
        let results = flat_top_k(&[1.0, 0.0], refs.into_iter(), 4, DistanceMetric::Cosine);
        for w in results.windows(2) {
            assert!(w[0].score >= w[1].score, "{} should be >= {}", w[0].score, w[1].score);
        }
    }
}
