use std::collections::HashMap;

use crate::store::VectorMatch;

#[derive(Debug, Clone, Copy)]
pub enum FusionStrategy {
    Rrf { k: f32 },
    Alpha { alpha: f32 },
}

impl Default for FusionStrategy {
    fn default() -> Self {
        Self::Rrf { k: 60.0 }
    }
}

#[must_use]
pub fn fuse(
    vector: &[VectorMatch],
    keyword: &[VectorMatch],
    k: usize,
    strategy: FusionStrategy,
) -> Vec<VectorMatch> {
    match strategy {
        FusionStrategy::Rrf { k: rrf_k } => fuse_rrf(vector, keyword, k, rrf_k),
        FusionStrategy::Alpha { alpha } => fuse_alpha(vector, keyword, k, alpha),
    }
}

fn fuse_rrf(
    vector: &[VectorMatch],
    keyword: &[VectorMatch],
    k: usize,
    rrf_k: f32,
) -> Vec<VectorMatch> {
    let mut scores: HashMap<&str, f32> = HashMap::new();

    for (rank, m) in vector.iter().enumerate() {
        *scores.entry(&m.id).or_insert(0.0) += 1.0 / (rrf_k + (rank + 1) as f32);
    }

    for (rank, m) in keyword.iter().enumerate() {
        *scores.entry(&m.id).or_insert(0.0) += 1.0 / (rrf_k + (rank + 1) as f32);
    }

    let mut results: Vec<VectorMatch> = scores
        .into_iter()
        .map(|(id, score)| VectorMatch {
            id: id.to_string(),
            score,
        })
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(k);
    results
}

fn min_max_normalize(matches: &[VectorMatch]) -> Vec<(String, f32)> {
    if matches.is_empty() {
        return Vec::new();
    }
    let min = matches
        .iter()
        .map(|m| m.score)
        .fold(f32::INFINITY, f32::min);
    let max = matches
        .iter()
        .map(|m| m.score)
        .fold(f32::NEG_INFINITY, f32::max);
    let range = max - min;

    matches
        .iter()
        .map(|m| {
            let norm = if range > 0.0 {
                (m.score - min) / range
            } else {
                1.0
            };
            (m.id.clone(), norm)
        })
        .collect()
}

fn fuse_alpha(
    vector: &[VectorMatch],
    keyword: &[VectorMatch],
    k: usize,
    alpha: f32,
) -> Vec<VectorMatch> {
    let alpha = alpha.clamp(0.0, 1.0);

    let vec_normed = min_max_normalize(vector);
    let kw_normed = min_max_normalize(keyword);

    let mut scores: HashMap<String, f32> = HashMap::new();

    for (id, norm) in &vec_normed {
        *scores.entry(id.clone()).or_insert(0.0) += alpha * norm;
    }

    for (id, norm) in &kw_normed {
        *scores.entry(id.clone()).or_insert(0.0) += (1.0 - alpha) * norm;
    }

    let mut results: Vec<VectorMatch> = scores
        .into_iter()
        .map(|(id, score)| VectorMatch { id, score })
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(k);
    results
}

pub trait Reranker: Send + Sync {
    fn rerank(
        &self,
        query: &str,
        candidates: Vec<VectorMatch>,
        texts: &[&str],
    ) -> Vec<VectorMatch>;
}

pub fn hybrid_search(
    query_text: &str,
    query_vector: &[f32],
    vector_store: &dyn crate::store::VectorStore,
    bm25: &crate::bm25::Bm25Index,
    k: usize,
    strategy: FusionStrategy,
    reranker: Option<&dyn Reranker>,
) -> Vec<VectorMatch> {
    let fetch_n = k * 2;

    let vector_results = vector_store
        .search(query_vector, fetch_n)
        .unwrap_or_default();
    let keyword_results = bm25.search(query_text, fetch_n);

    let mut fused = fuse(&vector_results, &keyword_results, k, strategy);

    if let Some(rr) = reranker {
        let texts: Vec<&str> = Vec::new();
        fused = rr.rerank(query_text, fused, &texts);
        fused.truncate(k);
    }

    fused
}
