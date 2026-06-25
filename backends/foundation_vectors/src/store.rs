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

#[async_trait::async_trait]
pub trait AsyncVectorStore: Send + Sync {
    async fn insert_async(
        &self,
        namespace: &str,
        entry: VectorEntry,
    ) -> Result<(), VectorStoreError>;

    async fn delete_async(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<(), VectorStoreError>;

    async fn search_async(
        &self,
        namespace: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorMatch>, VectorStoreError>;

    async fn get_async(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<Option<VectorEntry>, VectorStoreError>;

    async fn len_async(&self, namespace: &str) -> usize;

    fn config(&self) -> &VectorStoreConfig;
}

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
