pub mod types;
pub mod graph;
pub mod serial;

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

pub use graph::{CodeGraph, GraphError, Subgraph};
pub use serial::{GraphStore, InMemoryGraphStore};
#[cfg(feature = "code-graph")]
pub use lang_config::{extract_js_file, extract_py_file, extract_ts_file, LanguageConfig};
#[cfg(feature = "code-graph")]
pub use go_walker::extract_go_file;
#[cfg(feature = "code-graph")]
pub use cluster::{leiden, ClusteringResult};
pub use types::{
    Confidence, FileExtraction, GraphEdge, GraphNode, NodeKind, RawCall, Relation,
    file_stem_qualified, make_id, normalize_label,
};
