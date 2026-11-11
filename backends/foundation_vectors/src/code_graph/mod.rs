pub mod graph;
pub mod serial;
pub mod types;

#[cfg(feature = "code-graph")]
pub mod rust_walker;

#[cfg(feature = "code-graph")]
pub mod lang_config;

#[cfg(feature = "code-graph")]
pub mod go_walker;

#[cfg(feature = "code-graph")]
pub mod cluster;

#[cfg(feature = "code-graph")]
pub mod analyze;

#[cfg(feature = "code-graph")]
pub use cluster::{leiden, ClusteringResult};
#[cfg(feature = "code-graph")]
pub use go_walker::extract_go_file;
pub use graph::{CodeGraph, GraphError, Subgraph};
#[cfg(feature = "code-graph")]
pub use lang_config::{extract_js_file, extract_py_file, extract_ts_file, LanguageConfig};
pub use serial::{GraphStore, InMemoryGraphStore};
pub use types::{
    file_stem_qualified, make_id, normalize_label, Confidence, FileExtraction, GraphEdge,
    GraphNode, NodeKind, RawCall, Relation,
};
