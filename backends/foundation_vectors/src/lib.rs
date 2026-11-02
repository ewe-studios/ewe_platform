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
//!
//! The `sql-store` feature's `SqlVectorStore` is the portable fallback: it
//! scans a whole namespace per search (O(n·d) time). Use `LibsqlVectorStore`
//! or a dedicated vector backend for large namespaces; `SqlVectorStoreConfig`
//! sets the size at which one scan writes a `tracing` warning.

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::missing_panics_doc
)]
pub mod bm25;
#[cfg(any(feature = "code-graph", feature = "code-graph-query"))]
pub mod code_graph;
#[cfg(all(feature = "fjall-store", not(target_arch = "wasm32")))]
pub mod fjall_store;
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
#[cfg(feature = "sql-store")]
pub mod libsql_vector_store;
pub mod metric;
#[cfg(feature = "sql-store")]
pub mod schema;
#[cfg(feature = "sql-store")]
pub mod sql_store;
#[cfg(all(feature = "sqlite-vec", not(target_arch = "wasm32")))]
pub mod sqlite_vec_store;
pub mod sqrt;
pub mod store;
#[cfg(feature = "turbopuffer")]
pub mod turbopuffer;
pub mod vector;

pub use bm25::{Bm25Index, SimpleTokenizer, Tokenizer};
#[cfg(all(feature = "fjall-store", not(target_arch = "wasm32")))]
pub use fjall_store::{FjallVectorStore, SharedFjallVectorStore};
pub use flat::{flat_top_k, flat_top_k_owned, TopK};
pub use fusion::{fuse, hybrid_search, FusionStrategy, Reranker, TextLookup};
pub use hnsw::HnswIndex;
pub use index::{load_index, FlatIndex, VectorError, VectorIndex};
pub use ivf::IvfIndex;
#[cfg(feature = "sql-store")]
pub use libsql_vector_store::{LibsqlVectorStore, NamespaceStrategy};
pub use metric::{DistanceMetric, OrderedScore};
#[cfg(feature = "sql-store")]
pub use sql_store::{SqlVectorStore, SqlVectorStoreConfig};
#[cfg(all(feature = "sqlite-vec", not(target_arch = "wasm32")))]
pub use sqlite_vec_store::SqliteVecVectorStore;
pub use sqrt::SqrtStrategy;
pub use store::{
    InMemoryVectorStore, VectorEntry, VectorMatch, VectorStore, VectorStoreConfig, VectorStoreError,
};
#[cfg(feature = "turbopuffer")]
pub use turbopuffer::{TurboPufferConfig, TurboPufferLimits, TurboPufferVectorStore};
pub use vector::Vector;
