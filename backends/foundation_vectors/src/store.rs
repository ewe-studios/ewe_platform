//! `VectorStore` trait and in-memory backend.

use crate::flat::flat_top_k;
use crate::metric::DistanceMetric;
use crate::sqrt::SqrtStrategy;
use crate::vector::Vector;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

// ---------------------------------------------------------------------------
// Types

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorMatch {
    pub id: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorEntry {
    pub id: String,
    pub vector: Vector,
    pub metadata: VectorMetadata,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct VectorMetadata {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorStoreConfig {
    pub dimension: usize,
    pub metric: DistanceMetric,
    pub sqrt_strategy: SqrtStrategy,
}

impl VectorStoreConfig {
    #[must_use]
    pub fn new(dimension: usize, metric: DistanceMetric) -> Self {
        Self {
            dimension,
            metric,
            sqrt_strategy: SqrtStrategy::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Error

#[derive(Debug, Clone, PartialEq)]
pub enum VectorStoreError {
    DimensionMismatch { expected: usize, got: usize },
    ZeroVector,
    NotFound { id: String },
    Backend(String),
}

impl core::fmt::Display for VectorStoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DimensionMismatch { expected, got } => {
                write!(f, "dimension mismatch: expected {expected}, got {got}")
            }
            Self::ZeroVector => write!(f, "zero-magnitude vector rejected"),
            Self::NotFound { id } => write!(f, "vector not found: {id}"),
            Self::Backend(msg) => write!(f, "backend error: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// VectorStore trait

pub trait VectorStore: Send + Sync {
    fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), VectorStoreError>;

    fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError>;

    fn search(
        &self,
        namespace: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorMatch>, VectorStoreError>;

    fn get(&self, namespace: &str, id: &str) -> Result<Option<VectorEntry>, VectorStoreError>;

    fn len(&self, namespace: &str) -> usize;

    fn is_empty(&self, namespace: &str) -> bool {
        self.len(namespace) == 0
    }

    fn config(&self) -> &VectorStoreConfig;
}

// ---------------------------------------------------------------------------
// InMemoryVectorStore

pub struct InMemoryVectorStore {
    config: VectorStoreConfig,
    namespaces: RwLock<HashMap<String, HashMap<String, VectorEntry>>>,
}

impl InMemoryVectorStore {
    #[must_use]
    pub fn new(config: VectorStoreConfig) -> Self {
        Self {
            config,
            namespaces: RwLock::new(HashMap::new()),
        }
    }

    fn validate_and_prepare(&self, mut entry: VectorEntry) -> Result<VectorEntry, VectorStoreError> {
        if entry.vector.dimension() != self.config.dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.config.dimension,
                got: entry.vector.dimension(),
            });
        }
        if entry.vector.is_zero() {
            return Err(VectorStoreError::ZeroVector);
        }
        if self.config.sqrt_strategy == SqrtStrategy::NormalizedVectors {
            entry.vector = entry.vector.normalize().ok_or(VectorStoreError::ZeroVector)?;
        }
        Ok(entry)
    }
}

impl VectorStore for InMemoryVectorStore {
    fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), VectorStoreError> {
        let entry = self.validate_and_prepare(entry)?;
        let mut ns_map = self.namespaces.write().unwrap();
        ns_map
            .entry(namespace.to_string())
            .or_default()
            .insert(entry.id.clone(), entry);
        Ok(())
    }

    fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let mut ns_map = self.namespaces.write().unwrap();
        let entries = ns_map
            .get_mut(namespace)
            .ok_or_else(|| VectorStoreError::NotFound { id: id.to_string() })?;
        entries
            .remove(id)
            .ok_or_else(|| VectorStoreError::NotFound { id: id.to_string() })?;
        Ok(())
    }

    fn search(
        &self,
        namespace: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorMatch>, VectorStoreError> {
        if query.len() != self.config.dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.config.dimension,
                got: query.len(),
            });
        }
        let ns_map = self.namespaces.read().unwrap();
        let Some(entries) = ns_map.get(namespace) else {
            return Ok(Vec::new());
        };
        let iter = entries.values().map(|e| (e.id.as_str(), e.vector.as_slice()));
        Ok(flat_top_k(query, iter, k, self.config.metric))
    }

    fn get(&self, namespace: &str, id: &str) -> Result<Option<VectorEntry>, VectorStoreError> {
        let ns_map = self.namespaces.read().unwrap();
        let Some(entries) = ns_map.get(namespace) else {
            return Ok(None);
        };
        Ok(entries.get(id).cloned())
    }

    fn len(&self, namespace: &str) -> usize {
        let ns_map = self.namespaces.read().unwrap();
        ns_map.get(namespace).map_or(0, HashMap::len)
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NS: &str = "test";

    fn make_store() -> InMemoryVectorStore {
        InMemoryVectorStore::new(VectorStoreConfig::new(3, DistanceMetric::Cosine))
    }

    fn entry(id: &str, data: Vec<f32>) -> VectorEntry {
        VectorEntry {
            id: id.to_string(),
            vector: Vector::new(data),
            metadata: VectorMetadata::default(),
        }
    }

    #[test]
    fn insert_and_get() {
        let store = make_store();
        store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
        let got = store.get(NS, "a").unwrap().unwrap();
        assert_eq!(got.id, "a");
        assert_eq!(got.vector.data, vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn dimension_mismatch_rejected() {
        let store = make_store();
        let err = store.insert(NS, entry("bad", vec![1.0, 0.0])).unwrap_err();
        assert!(matches!(err, VectorStoreError::DimensionMismatch { expected: 3, got: 2 }));
    }

    #[test]
    fn zero_vector_rejected() {
        let store = make_store();
        let err = store.insert(NS, entry("zero", vec![0.0, 0.0, 0.0])).unwrap_err();
        assert!(matches!(err, VectorStoreError::ZeroVector));
    }

    #[test]
    fn search_top_k() {
        let store = make_store();
        store.insert(NS, entry("x", vec![1.0, 0.0, 0.0])).unwrap();
        store.insert(NS, entry("y", vec![0.0, 1.0, 0.0])).unwrap();
        store.insert(NS, entry("z", vec![0.9, 0.1, 0.0])).unwrap();

        let results = store.search(NS, &[1.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "x");
        assert_eq!(results[1].id, "z");
    }

    #[test]
    fn search_dimension_mismatch() {
        let store = make_store();
        store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
        let err = store.search(NS, &[1.0, 0.0], 1).unwrap_err();
        assert!(matches!(err, VectorStoreError::DimensionMismatch { .. }));
    }

    #[test]
    fn delete_entry() {
        let store = make_store();
        store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
        assert_eq!(store.len(NS), 1);
        store.delete(NS, "a").unwrap();
        assert_eq!(store.len(NS), 0);
        assert!(store.get(NS, "a").unwrap().is_none());
    }

    #[test]
    fn delete_not_found() {
        let store = make_store();
        let err = store.delete(NS, "nope").unwrap_err();
        assert!(matches!(err, VectorStoreError::NotFound { .. }));
    }

    #[test]
    fn overwrite_on_insert() {
        let store = make_store();
        store.insert(NS, entry("a", vec![1.0, 0.0, 0.0])).unwrap();
        store.insert(NS, entry("a", vec![0.0, 1.0, 0.0])).unwrap();
        let got = store.get(NS, "a").unwrap().unwrap();
        assert_eq!(got.vector.data, vec![0.0, 1.0, 0.0]);
        assert_eq!(store.len(NS), 1);
    }

    #[test]
    fn empty_store() {
        let store = make_store();
        assert!(store.is_empty(NS));
        let results = store.search(NS, &[1.0, 0.0, 0.0], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn namespace_isolation() {
        let store = make_store();
        store.insert("ns_a", entry("v1", vec![1.0, 0.0, 0.0])).unwrap();
        store.insert("ns_b", entry("v2", vec![0.0, 1.0, 0.0])).unwrap();

        assert_eq!(store.len("ns_a"), 1);
        assert_eq!(store.len("ns_b"), 1);

        let results = store.search("ns_a", &[1.0, 0.0, 0.0], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "v1");

        assert!(store.get("ns_a", "v2").unwrap().is_none());
        assert!(store.get("ns_b", "v1").unwrap().is_none());
    }

    #[test]
    fn normalized_vectors_strategy() {
        let config = VectorStoreConfig {
            dimension: 2,
            metric: DistanceMetric::Cosine,
            sqrt_strategy: SqrtStrategy::NormalizedVectors,
        };
        let store = InMemoryVectorStore::new(config);
        store.insert(NS, entry("a", vec![3.0, 4.0])).unwrap();
        let got = store.get(NS, "a").unwrap().unwrap();
        let mag = got.vector.magnitude();
        assert!((mag - 1.0).abs() < 1e-5, "stored vector should be normalized, mag={mag}");
    }

    #[test]
    fn l2_search() {
        let config = VectorStoreConfig::new(2, DistanceMetric::L2);
        let store = InMemoryVectorStore::new(config);
        store.insert(NS, entry("near", vec![0.1, 0.0])).unwrap();
        store.insert(NS, entry("far", vec![10.0, 10.0])).unwrap();
        let results = store.search(NS, &[0.0, 0.0], 1).unwrap();
        assert_eq!(results[0].id, "near");
    }

    #[test]
    fn dot_search() {
        let config = VectorStoreConfig::new(2, DistanceMetric::Dot);
        let store = InMemoryVectorStore::new(config);
        store.insert(NS, entry("high", vec![10.0, 10.0])).unwrap();
        store.insert(NS, entry("low", vec![0.1, 0.1])).unwrap();
        let results = store.search(NS, &[1.0, 1.0], 1).unwrap();
        assert_eq!(results[0].id, "high");
    }
}
