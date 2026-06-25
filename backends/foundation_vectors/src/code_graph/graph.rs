use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;

use super::types::{
    Confidence, FileExtraction, GraphEdge, GraphNode, RawCall, Relation,
    normalize_label,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    NodeNotFound(String),
    ParseError(String),
    IoError(String),
}

impl core::fmt::Display for GraphError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NodeNotFound(id) => write!(f, "node not found: {id}"),
            Self::ParseError(msg) => write!(f, "parse error: {msg}"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Subgraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<(String, String, GraphEdge)>,
    pub token_estimate: usize,
}

pub struct CodeGraph {
    pub(crate) graph: DiGraph<GraphNode, GraphEdge>,
    pub(crate) id_to_index: HashMap<String, NodeIndex>,
    pub(crate) label_index: HashMap<String, Vec<NodeIndex>>,
}

impl CodeGraph {
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            id_to_index: HashMap::new(),
            label_index: HashMap::new(),
        }
    }

    pub fn build(extractions: Vec<FileExtraction>) -> Self {
        let mut cg = Self::new();

        for extraction in &extractions {
            for node in &extraction.nodes {
                cg.add_node(node.clone());
            }
            for (src, tgt, edge) in &extraction.edges {
                cg.add_edge(src, tgt, edge.clone());
            }
        }

        let all_raw_calls: Vec<RawCall> = extractions
            .into_iter()
            .flat_map(|e| e.raw_calls)
            .collect();
        cg.resolve_cross_file_calls(&all_raw_calls);

        cg.deduplicate_by_label();

        cg
    }

    pub fn add_node(&mut self, node: GraphNode) {
        if self.id_to_index.contains_key(&node.id) {
            return;
        }
        let id = node.id.clone();
        let label_key = normalize_label(&node.label);
        let idx = self.graph.add_node(node);
        self.id_to_index.insert(id, idx);
        self.label_index.entry(label_key).or_default().push(idx);
    }

    pub fn add_edge(&mut self, source: &str, target: &str, edge: GraphEdge) {
        let Some(&src_idx) = self.id_to_index.get(source) else {
            return;
        };
        let Some(&tgt_idx) = self.id_to_index.get(target) else {
            return;
        };
        if src_idx == tgt_idx {
            return;
        }
        self.graph.add_edge(src_idx, tgt_idx, edge);
    }

    fn resolve_cross_file_calls(&mut self, raw_calls: &[RawCall]) {
        let mut seen_pairs: HashMap<(String, String), bool> = HashMap::new();

        for call in raw_calls {
            if call.is_member_call {
                continue;
            }
            let label_key = normalize_label(&call.callee_name);
            let targets: Vec<NodeIndex> = self
                .label_index
                .get(&label_key)
                .cloned()
                .unwrap_or_default();

            for &tgt_idx in &targets {
                let tgt_id = self.graph[tgt_idx].id.clone();
                let pair = (call.caller_id.clone(), tgt_id.clone());
                if seen_pairs.contains_key(&pair) {
                    continue;
                }
                seen_pairs.insert(pair, true);

                let edge = GraphEdge {
                    relation: Relation::Calls,
                    confidence: Confidence::Inferred,
                    source_file: call.source_file.clone(),
                    source_line: call.source_line,
                    weight: 0.8,
                };
                self.add_edge(&call.caller_id, &tgt_id, edge);
            }
        }
    }

    fn deduplicate_by_label(&mut self) {
        let mut label_groups: HashMap<String, Vec<NodeIndex>> = HashMap::new();
        for (_key, idx) in &self.id_to_index {
            let node = &self.graph[*idx];
            let key = normalize_label(&node.label);
            label_groups.entry(key).or_default().push(*idx);
        }

        for (_label, indices) in &label_groups {
            if indices.len() <= 1 {
                continue;
            }
            let canonical = indices[0];
            for &dup in &indices[1..] {
                if self.graph[canonical].kind != self.graph[dup].kind {
                    continue;
                }
                let incoming: Vec<_> = self
                    .graph
                    .edges_directed(dup, Direction::Incoming)
                    .map(|e| (e.source(), e.weight().clone()))
                    .collect();
                let outgoing: Vec<_> = self
                    .graph
                    .edges_directed(dup, Direction::Outgoing)
                    .map(|e| (e.target(), e.weight().clone()))
                    .collect();

                for (src, edge) in incoming {
                    if src != canonical {
                        self.graph.add_edge(src, canonical, edge);
                    }
                }
                for (tgt, edge) in outgoing {
                    if tgt != canonical {
                        self.graph.add_edge(canonical, tgt, edge);
                    }
                }
            }
        }
    }

    // -- Query methods --------------------------------------------------------

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn get_node(&self, id: &str) -> Option<&GraphNode> {
        self.id_to_index.get(id).map(|&idx| &self.graph[idx])
    }

    pub fn find_entity(&self, name: &str) -> Vec<&GraphNode> {
        let key = normalize_label(name);
        let mut results = Vec::new();

        if let Some(indices) = self.label_index.get(&key) {
            for &idx in indices {
                results.push(&self.graph[idx]);
            }
        }

        if results.is_empty() {
            for idx in self.graph.node_indices() {
                let node = &self.graph[idx];
                let node_label = normalize_label(&node.label);
                if node_label.contains(&key) {
                    results.push(node);
                }
            }
        }

        results
    }

    pub fn callers_of(&self, id: &str) -> Vec<&GraphNode> {
        let Some(&idx) = self.id_to_index.get(id) else {
            return Vec::new();
        };
        self.graph
            .edges_directed(idx, Direction::Incoming)
            .filter(|e| e.weight().relation == Relation::Calls)
            .map(|e| &self.graph[e.source()])
            .collect()
    }

    pub fn callees_of(&self, id: &str) -> Vec<&GraphNode> {
        let Some(&idx) = self.id_to_index.get(id) else {
            return Vec::new();
        };
        self.graph
            .edges_directed(idx, Direction::Outgoing)
            .filter(|e| e.weight().relation == Relation::Calls)
            .map(|e| &self.graph[e.target()])
            .collect()
    }

    pub fn neighborhood(&self, id: &str, budget_tokens: usize) -> Result<Subgraph, GraphError> {
        let Some(&start) = self.id_to_index.get(id) else {
            return Err(GraphError::NodeNotFound(id.to_string()));
        };

        let mut visited: HashMap<NodeIndex, bool> = HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        let mut result_nodes = Vec::new();
        let mut result_edges: Vec<(String, String, GraphEdge)> = Vec::new();
        let mut tokens_used: usize = 0;

        queue.push_back((start, 0_usize));
        visited.insert(start, true);

        while let Some((current, depth)) = queue.pop_front() {
            let node = &self.graph[current];
            let node_tokens = estimate_node_tokens(node);
            if tokens_used + node_tokens > budget_tokens && !result_nodes.is_empty() {
                break;
            }
            tokens_used += node_tokens;
            result_nodes.push(node.clone());

            if depth >= 2 {
                continue;
            }

            for edge_ref in self.graph.edges_directed(current, Direction::Outgoing) {
                let neighbor = edge_ref.target();
                if !visited.contains_key(&neighbor) {
                    visited.insert(neighbor, true);
                    queue.push_back((neighbor, depth + 1));
                    result_edges.push((
                        self.graph[current].id.clone(),
                        self.graph[neighbor].id.clone(),
                        edge_ref.weight().clone(),
                    ));
                }
            }
            for edge_ref in self.graph.edges_directed(current, Direction::Incoming) {
                let neighbor = edge_ref.source();
                if !visited.contains_key(&neighbor) {
                    visited.insert(neighbor, true);
                    queue.push_back((neighbor, depth + 1));
                    result_edges.push((
                        self.graph[neighbor].id.clone(),
                        self.graph[current].id.clone(),
                        edge_ref.weight().clone(),
                    ));
                }
            }
        }

        Ok(Subgraph {
            nodes: result_nodes,
            edges: result_edges,
            token_estimate: tokens_used,
        })
    }

    pub fn shortest_path(&self, from: &str, to: &str) -> Option<Vec<String>> {
        let &start = self.id_to_index.get(from)?;
        let &end = self.id_to_index.get(to)?;

        let path = petgraph::algo::astar(
            &self.graph,
            start,
            |n| n == end,
            |_| 1u32,
            |_| 0u32,
        );

        path.map(|(_cost, indices)| {
            indices
                .into_iter()
                .map(|idx| self.graph[idx].id.clone())
                .collect()
        })
    }

    pub fn nodes_by_file(&self, file: &str) -> Vec<&GraphNode> {
        self.graph
            .node_indices()
            .map(|idx| &self.graph[idx])
            .filter(|n| n.source_file == file)
            .collect()
    }

    pub fn all_nodes(&self) -> Vec<&GraphNode> {
        self.graph.node_indices().map(|idx| &self.graph[idx]).collect()
    }
}

impl Default for CodeGraph {
    fn default() -> Self {
        Self::new()
    }
}

fn estimate_node_tokens(node: &GraphNode) -> usize {
    10 + node.label.len() / 4
        + node.source_file.len() / 4
        + node.rationale.as_ref().map_or(0, |r| r.len() / 4)
}
