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

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::missing_panics_doc
)]
pub mod bm25;
pub mod flat;
#[allow(clippy::cast_precision_loss)]
pub mod fusion;
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub mod hnsw;
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]
pub mod index;
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub mod ivf;
pub mod metric;
pub mod sqrt;
pub mod store;
#[cfg(any(feature = "code-graph", feature = "code-graph-query"))]
pub mod code_graph;
#[cfg(feature = "sql-store")]
pub mod schema;
#[cfg(feature = "sql-store")]
pub mod sql_store;
#[cfg(all(feature = "fjall-store", not(target_arch = "wasm32")))]
pub mod fjall_store;
#[cfg(feature = "turbopuffer")]
pub mod turbopuffer;
pub mod vector;

pub use bm25::{Bm25Index, SimpleTokenizer, Tokenizer};
pub use flat::{flat_top_k, flat_top_k_owned};
pub use fusion::{FusionStrategy, Reranker, fuse, hybrid_search};
pub use hnsw::HnswIndex;
pub use index::{FlatIndex, VectorError, VectorIndex, load_index};
pub use ivf::IvfIndex;
pub use metric::{DistanceMetric, OrderedScore};
pub use sqrt::SqrtStrategy;
pub use store::{
    AsyncVectorStore, InMemoryVectorStore, VectorEntry, VectorMatch, VectorStore,
    VectorStoreConfig, VectorStoreError,
};
#[cfg(feature = "sql-store")]
pub use sql_store::SqlVectorStore;
#[cfg(all(feature = "fjall-store", not(target_arch = "wasm32")))]
pub use fjall_store::{FjallVectorStore, SharedFjallVectorStore};
#[cfg(feature = "turbopuffer")]
pub use turbopuffer::TurboPufferVectorStore;
pub use vector::Vector;
