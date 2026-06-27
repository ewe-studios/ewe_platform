//! Graph analysis: god nodes, surprising connections (F27c).
//!
//! WHY: After clustering (Doc 06), the agent needs actionable insights:
//! "what are the central hubs?", "what unexpected connections exist between
//! communities?". These are the outputs that make a graph query far more
//! informative than reading raw files.
//!
//! WHAT: `god_nodes()` (top-k by in+out degree), `surprising_connections()`
//! (edges crossing community boundaries, optionally filtered by minimum weight).

use std::collections::HashMap;

use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use super::graph::CodeGraph;
use super::types::{GraphEdge, GraphNode};

/// A node ranked by centrality (degree).
#[derive(Debug, Clone)]
pub struct GodNode {
    pub id: String,
    pub label: String,
    pub source_file: String,
    pub in_degree: usize,
    pub out_degree: usize,
    pub total_degree: usize,
}

/// An edge that crosses a community boundary.
#[derive(Debug, Clone)]
pub struct SurprisingConnection {
    pub source_id: String,
    pub source_label: String,
    pub target_id: String,
    pub target_label: String,
    pub edge: GraphEdge,
    pub source_community: usize,
    pub target_community: usize,
}

impl CodeGraph {
    /// Return the top-k nodes by total degree (in + out), the "god nodes" or
    /// central hubs of the graph.
    ///
    /// High in-degree = many things depend on this node. High out-degree = this
    /// node depends on many things. Both are structurally significant.
    pub fn god_nodes(&self, k: usize) -> Vec<GodNode> {
        let mut scored: Vec<GodNode> = Vec::with_capacity(self.graph.node_count());

        for idx in self.graph.node_indices() {
            let node = &self.graph[idx];
            let in_degree = self.graph.edges_directed(idx, petgraph::Direction::Incoming).count();
            let out_degree = self.graph.edges_directed(idx, petgraph::Direction::Outgoing).count();
            scored.push(GodNode {
                id: node.id.clone(),
                label: node.label.clone(),
                source_file: node.source_file.clone(),
                in_degree,
                out_degree,
                total_degree: in_degree + out_degree,
            });
        }

        scored.sort_by_key(|n| std::cmp::Reverse(n.total_degree));
        scored.truncate(k);
        scored
    }

    /// Find edges that cross community boundaries. Returns connections between
    /// nodes in different communities, sorted by source-community size (largest
    /// first) for stable output.
    ///
    /// If `communities` is None, returns an empty vec (no clustering has been
    /// performed). A community map maps node id → community id (0 = largest).
    pub fn surprising_connections(
        &self,
        communities: Option<&HashMap<String, usize>>,
    ) -> Vec<SurprisingConnection> {
        let Some(comm) = communities else {
            return Vec::new();
        };

        let mut result = Vec::new();

        for edge_idx in self.graph.edge_indices() {
            let (src, tgt) = self.graph.edge_endpoints(edge_idx).unwrap();
            let src_node = &self.graph[src];
            let tgt_node = &self.graph[tgt];
            let edge = &self.graph[edge_idx];

            let src_comm = comm.get(&src_node.id).copied().unwrap_or(usize::MAX);
            let tgt_comm = comm.get(&tgt_node.id).copied().unwrap_or(usize::MAX);

            if src_comm != tgt_comm
                && src_comm != usize::MAX
                && tgt_comm != usize::MAX
            {
                result.push(SurprisingConnection {
                    source_id: src_node.id.clone(),
                    source_label: src_node.label.clone(),
                    target_id: tgt_node.id.clone(),
                    target_label: tgt_node.label.clone(),
                    edge: edge.clone(),
                    source_community: src_comm,
                    target_community: tgt_comm,
                });
            }
        }

        // Sort by source community (0 = largest first), then target
        result.sort_by(|a, b| {
            a.source_community.cmp(&b.source_community)
                .then_with(|| a.target_community.cmp(&b.target_community))
        });
        result
    }
}

/// Community membership result from Leiden/Louvain clustering.
#[derive(Debug, Clone)]
pub struct ClusteringResult {
    /// Maps node id → community id (0 = largest community).
    pub communities: HashMap<String, usize>,
    /// Number of communities found.
    pub num_communities: usize,
    /// Modularity score of the partition (higher = better clustering).
    pub modularity: f64,
}
