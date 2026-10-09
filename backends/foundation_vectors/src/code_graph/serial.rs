use serde::{Deserialize, Serialize};

use super::graph::{CodeGraph, GraphError};
use super::types::{GraphEdge, GraphNode};

#[derive(Serialize, Deserialize)]
struct SerializedGraph {
    nodes: Vec<GraphNode>,
    edges: Vec<SerializedEdge>,
}

#[derive(Serialize, Deserialize)]
struct SerializedEdge {
    source: String,
    target: String,
    #[serde(flatten)]
    edge: GraphEdge,
}

impl CodeGraph {
    pub fn to_json(&self) -> Result<String, GraphError> {
        let nodes: Vec<GraphNode> = self
            .graph
            .node_indices()
            .map(|idx| self.graph[idx].clone())
            .collect();

        let edges: Vec<SerializedEdge> = self
            .graph
            .edge_indices()
            .filter_map(|eidx| {
                let (src, tgt) = self.graph.edge_endpoints(eidx)?;
                let edge = self.graph[eidx].clone();
                Some(SerializedEdge {
                    source: self.graph[src].id.clone(),
                    target: self.graph[tgt].id.clone(),
                    edge,
                })
            })
            .collect();

        let sg = SerializedGraph { nodes, edges };
        serde_json::to_string_pretty(&sg)
            .map_err(|e| GraphError::ParseError(format!("serialize: {e}")))
    }

    pub fn from_json(json: &str) -> Result<Self, GraphError> {
        let sg: SerializedGraph = serde_json::from_str(json)
            .map_err(|e| GraphError::ParseError(format!("deserialize: {e}")))?;

        let mut cg = Self::new();
        for node in sg.nodes {
            cg.add_node(node);
        }
        for se in sg.edges {
            cg.add_edge(&se.source, &se.target, se.edge);
        }
        Ok(cg)
    }

    /// Serialize the graph to bytes for a [`GraphStore`] backend (OD-27-6).
    ///
    /// Graph metadata is not columnar, so this is plain serde/JSON (the
    /// human-inspectable `graph.json` parity), just as bytes. A wasm deployment
    /// loads these bytes and queries — it never rebuilds.
    pub fn to_bytes(&self) -> Result<Vec<u8>, GraphError> {
        Ok(self.to_json()?.into_bytes())
    }

    /// Reconstruct a (query-only) graph from [`Self::to_bytes`] output.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, GraphError> {
        let json = core::str::from_utf8(bytes)
            .map_err(|e| GraphError::ParseError(format!("graph bytes not UTF-8: {e}")))?;
        Self::from_json(json)
    }
}

/// Persistence backend for a built `CodeGraph` (OD-27-6).
///
/// The graph exposes `to_bytes`/`from_bytes`; a `GraphStore` just persists the
/// opaque bytes under a key. Implementations range from in-memory (tests, small
/// repos) to fjall / `DocumentStore` (large repos) — the graph never depends on
/// which one. Keeping it a trait means the native builder and a wasm query
/// deployment can share the same persisted artifact.
pub trait GraphStore {
    /// Persist `bytes` under `key`, replacing any previous value.
    fn save(&mut self, key: &str, bytes: &[u8]) -> Result<(), GraphError>;
    /// Load the bytes stored under `key`, or `None` if absent.
    fn load(&self, key: &str) -> Result<Option<Vec<u8>>, GraphError>;

    /// Convenience: persist a graph under `key`.
    fn save_graph(&mut self, key: &str, graph: &CodeGraph) -> Result<(), GraphError> {
        let bytes = graph.to_bytes()?;
        self.save(key, &bytes)
    }

    /// Convenience: load and rebuild a graph stored under `key`.
    fn load_graph(&self, key: &str) -> Result<Option<CodeGraph>, GraphError> {
        match self.load(key)? {
            Some(bytes) => Ok(Some(CodeGraph::from_bytes(&bytes)?)),
            None => Ok(None),
        }
    }
}

/// In-memory [`GraphStore`] — the default backend (and the one tests use).
#[derive(Debug, Default)]
pub struct InMemoryGraphStore {
    map: std::collections::HashMap<String, Vec<u8>>,
}

impl InMemoryGraphStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl GraphStore for InMemoryGraphStore {
    fn save(&mut self, key: &str, bytes: &[u8]) -> Result<(), GraphError> {
        self.map.insert(key.to_string(), bytes.to_vec());
        Ok(())
    }

    fn load(&self, key: &str) -> Result<Option<Vec<u8>>, GraphError> {
        Ok(self.map.get(key).cloned())
    }
}
