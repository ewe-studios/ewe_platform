//! Dense vector distance metrics, flat-scan top-k, and `VectorStore` trait.
//!
//! WHY: Vector search algorithms live here (owned by us) so behavior is
//! consistent across every backend, there's always a fallback when a DB
//! lacks native vector search, and it works in WASM.
//!
//! WHAT: `Vector`, `DistanceMetric` (cosine/L2/dot), `SqrtStrategy`,
//! `OrderedScore`, `flat_top_k`, `VectorStore` trait + in-memory backend.
//!
//! HOW: Pure `f32` arithmetic, `libm::sqrtf` for portability, bounded
//! min-heap for top-k, higher-is-better score convention throughout.

pub mod flat;
pub mod metric;
pub mod sqrt;
pub mod store;
pub mod vector;

pub use flat::{flat_top_k, flat_top_k_owned};
pub use metric::{DistanceMetric, OrderedScore};
pub use sqrt::SqrtStrategy;
pub use store::{InMemoryVectorStore, VectorEntry, VectorMatch, VectorStore, VectorStoreConfig, VectorStoreError};
pub use vector::Vector;
