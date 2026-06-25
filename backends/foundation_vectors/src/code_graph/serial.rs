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
}
