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
    /// The backend cannot do this operation or use this configuration (for
    /// example, a distance metric that the service does not have).
    Unsupported(String),
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
            Self::Unsupported(msg) => write!(f, "unsupported: {msg}"),
            Self::Backend(msg) => write!(f, "backend error: {msg}"),
        }
    }
}

impl std::error::Error for VectorStoreError {}

// ---------------------------------------------------------------------------
// VectorStore trait

/// A store of dense vectors, split into namespaces.
///
/// The trait is async only. A store that does input/output (I/O) awaits it,
/// so a valtron worker or a Cloudflare Worker never blocks on it.
///
/// Every store obeys the same contract (the crate's `tests/contract` module
/// checks it for each store):
///
/// - `insert` rejects a vector of the wrong dimension
///   ([`VectorStoreError::DimensionMismatch`]) and a zero vector
///   ([`VectorStoreError::ZeroVector`]). An insert with an existing id
///   replaces the entry.
/// - `delete` of an id that is not in the namespace returns
///   [`VectorStoreError::NotFound`].
/// - `search` returns at most `k` matches, best first (higher score
///   is closer). An unknown namespace gives no matches.
/// - `get` of an unknown id or namespace returns `Ok(None)`.
/// - `len` counts the entries of one namespace. A backend failure is
///   an error, never a count of 0.
/// - Namespaces are isolated: one namespace never sees another's entries.
#[async_trait::async_trait]
pub trait VectorStore: Send + Sync {
    async fn insert(
        &self,
        namespace: &str,
        entry: VectorEntry,
    ) -> Result<(), VectorStoreError>;

    async fn delete(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<(), VectorStoreError>;

    async fn search(
        &self,
        namespace: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<VectorMatch>, VectorStoreError>;

    async fn get(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<Option<VectorEntry>, VectorStoreError>;

    async fn len(&self, namespace: &str) -> Result<usize, VectorStoreError>;

    async fn is_empty(&self, namespace: &str) -> Result<bool, VectorStoreError> {
        Ok(self.len(namespace).await? == 0)
    }

    fn config(&self) -> &VectorStoreConfig;
}

// ---------------------------------------------------------------------------
// InMemoryVectorStore

/// A vector store in process memory. It does no I/O, so every future is
/// ready on its first poll.
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

/// The error for a lock that a panicking thread poisoned.
fn poisoned<T>(_: std::sync::PoisonError<T>) -> VectorStoreError {
    VectorStoreError::Backend("in-memory vector store lock poisoned".into())
}

#[async_trait::async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn insert(
        &self,
        namespace: &str,
        entry: VectorEntry,
    ) -> Result<(), VectorStoreError> {
        let entry = self.validate_and_prepare(entry)?;
        let mut ns_map = self.namespaces.write().map_err(poisoned)?;
        ns_map
            .entry(namespace.to_string())
            .or_default()
            .insert(entry.id.clone(), entry);
        Ok(())
    }

    async fn delete(&self, namespace: &str, id: &str) -> Result<(), VectorStoreError> {
        let mut ns_map = self.namespaces.write().map_err(poisoned)?;
        let entries = ns_map
            .get_mut(namespace)
            .ok_or_else(|| VectorStoreError::NotFound { id: id.to_string() })?;
        entries
            .remove(id)
            .ok_or_else(|| VectorStoreError::NotFound { id: id.to_string() })?;
        Ok(())
    }

    async fn search(
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
        let ns_map = self.namespaces.read().map_err(poisoned)?;
        let Some(entries) = ns_map.get(namespace) else {
            return Ok(Vec::new());
        };
        let iter = entries.values().map(|e| (e.id.as_str(), e.vector.as_slice()));
        Ok(flat_top_k(query, iter, k, self.config.metric))
    }

    async fn get(
        &self,
        namespace: &str,
        id: &str,
    ) -> Result<Option<VectorEntry>, VectorStoreError> {
        let ns_map = self.namespaces.read().map_err(poisoned)?;
        let Some(entries) = ns_map.get(namespace) else {
            return Ok(None);
        };
        Ok(entries.get(id).cloned())
    }

    async fn len(&self, namespace: &str) -> Result<usize, VectorStoreError> {
        let ns_map = self.namespaces.read().map_err(poisoned)?;
        Ok(ns_map.get(namespace).map_or(0, HashMap::len))
    }

    fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}
