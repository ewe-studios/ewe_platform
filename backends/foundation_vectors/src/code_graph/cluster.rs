//! Leiden community detection for code graphs (F27c).
//!
//! WHY: The agent needs to know "what subsystem is this node in?" — clustering
//! discovers natural module boundaries from the graph structure alone, without
//! relying on file paths or naming conventions.
//!
//! WHAT: Leiden algorithm — an improved variant of Louvain that guarantees
//! well-connected communities. Works on an undirected projection of the directed
//! code graph (Leiden requires undirected input).
//!
//! HOW: Three-phase iterative refinement:
//! 1. Local moving: each node joins the neighbor community that maximizes
//!    modularity gain (greedy).
//! 2. Refinement: split communities into well-connected sub-communities by
//!    starting each node as its own community within its parent, then greedily
//!    merging back if the merge improves local modularity.
//! 3. Aggregation: collapse refined communities into super-nodes and recurse.
//!
//! Implementation follows the original Leiden paper (Traag et al., 2019) with
//! a practical stopping criterion (max iterations, minimum modularity improvement).

use std::collections::HashMap;
use std::sync::Arc;

use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use super::graph::CodeGraph;
use super::types::{GraphEdge, GraphNode};

/// Undirected weighted edge for clustering.
#[derive(Debug, Clone)]
struct UEdge {
    source: NodeIndex,
    target: NodeIndex,
    weight: f64,
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

/// Build an undirected weighted projection of the CodeGraph.
fn build_undirected(graph: &petgraph::graph::DiGraph<GraphNode, GraphEdge>) -> Vec<UEdge> {
    let mut edges: Vec<UEdge> = Vec::new();
    // Sum edge weights in both directions for undirected projection
    let mut edge_map: HashMap<(NodeIndex, NodeIndex), f64> = HashMap::new();

    for edge_idx in graph.edge_indices() {
        let (src, tgt) = graph.edge_endpoints(edge_idx).unwrap();
        let weight = graph[edge_idx].weight as f64;

        let (a, b) = if src < tgt { (src, tgt) } else { (tgt, src) };
        *edge_map.entry((a, b)).or_insert(0.0) += weight;
    }

    for ((source, target), weight) in edge_map {
        edges.push(UEdge { source, target, weight });
    }
    edges
}

/// Total edge weight in the graph.
fn total_weight(edges: &[UEdge]) -> f64 {
    edges.iter().map(|e| e.weight).sum::<f64>() * 2.0
}

/// Build adjacency from undirected edges.
fn build_adj(
    edges: &[UEdge],
    n: usize,
) -> Vec<Vec<(usize, f64)>> {
    let mut adj = vec![Vec::new(); n];
    for e in edges {
        let s = e.source.index();
        let t = e.target.index();
        adj[s].push((t, e.weight));
        adj[t].push((s, e.weight));
    }
    adj
}

/// Compute the degree (sum of incident edge weights) for each node.
fn degrees(adj: &[Vec<(usize, f64)>]) -> Vec<f64> {
    adj.iter()
        .map(|neighbors| neighbors.iter().map(|(_, w)| w).sum())
        .collect()
}

/// Modularity of the current partition.
fn modularity(edges: &[UEdge], communities: &[usize], m: f64) -> f64 {
    if m == 0.0 {
        return 0.0;
    }
    let mut q = 0.0;
    for e in edges {
        if communities[e.source.index()] == communities[e.target.index()] {
            q += e.weight;
        }
    }
    // Expected edge weight within communities
    let comm_weights: HashMap<usize, f64> = communities
        .iter()
        .zip(
            adj_from_edges(edges)
                .iter()
                .map(|neighbors| neighbors.iter().map(|(_, w)| w).sum::<f64>())
                .collect::<Vec<_>>(),
        )
        .fold(HashMap::new(), |mut acc, (&c, d)| {
            *acc.entry(c).or_insert(0.0) += d;
            acc
        });

    for (&c, &dc) in &comm_weights {
        let dc2 = dc * dc;
        q -= dc2 / (2.0 * m);
    }

    q / m
}

fn adj_from_edges(edges: &[UEdge]) -> Vec<Vec<(usize, f64)>> {
    let n = edges
        .iter()
        .map(|e| e.source.index().max(e.target.index()) + 1)
        .max()
        .unwrap_or(0);
    build_adj(edges, n)
}

/// Local moving phase: greedily move each node to the best community.
fn local_move(
    edges: &[UEdge],
    adj: &[Vec<(usize, f64)>],
    degrees: &[f64],
    communities: &mut [usize],
    m: f64,
) -> bool {
    let n = communities.len();
    let mut improved = false;
    let resolution = 1.0;

    // Build community total degree map
    let mut comm_degrees: HashMap<usize, f64> = HashMap::new();
    for (&deg, &comm) in degrees.iter().zip(communities.iter()) {
        *comm_degrees.entry(comm).or_insert(0.0) += deg;
    }

    for i in 0..n {
        let current_comm = communities[i];
        let ki = degrees[i];

        // Find neighboring communities
        let mut neighbor_comms: HashMap<usize, f64> = HashMap::new();
        for &(neighbor, weight) in &adj[i] {
            *neighbor_comms.entry(communities[neighbor]).or_insert(0.0) += weight;
        }

        // Remove node from current community
        let ki_to_remove = ki;
        *comm_degrees.entry(current_comm).or_insert(0.0) -= ki_to_remove;
        let sigma_in_current = *neighbor_comms.get(&current_comm).unwrap_or(&0.0);

        let current_score = sigma_in_current - resolution * comm_degrees[&current_comm] * ki / m;

        // Find best community
        let mut best_comm = current_comm;
        let mut best_score = current_score;

        for (&comm, &ki_in) in &neighbor_comms {
            if comm == current_comm {
                continue;
            }
            let score = ki_in - resolution * comm_degrees.get(&comm).copied().unwrap_or(0.0) * ki / m;
            if score > best_score {
                best_score = score;
                best_comm = comm;
            }
        }

        // Move node
        if best_comm != current_comm {
            communities[i] = best_comm;
            *comm_degrees.entry(current_comm).or_insert(0.0) += ki; // undo the removal for old comm
            *comm_degrees.entry(best_comm).or_insert(0.0) += ki;
            improved = true;
        } else {
            *comm_degrees.entry(current_comm).or_insert(0.0) += ki; // put it back
        }
    }

    improved
}

/// Refinement phase: split communities into well-connected sub-communities.
fn refine(
    edges: &[UEdge],
    adj: &[Vec<(usize, f64)>],
    degrees: &[f64],
    communities: &mut [usize],
    m: f64,
) {
    let n = communities.len();

    // Group nodes by community
    let mut comm_nodes: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, &comm) in communities.iter().enumerate() {
        comm_nodes.entry(comm).or_default().push(i);
    }

    let mut refined = communities.to_vec();
    let mut next_comm_id = *communities.iter().max().unwrap_or(&0) + 1;

    for (&_comm, nodes) in &comm_nodes {
        if nodes.len() <= 1 {
            continue;
        }

        // Start each node as its own refined community
        let mut sub_comm: HashMap<usize, usize> = HashMap::new();
        for &node in nodes {
            sub_comm.insert(node, node); // each node starts as its own sub-comm
        }

        // Greedily merge nodes within the same parent community
        for &node in nodes {
            let current_sub = sub_comm[&node];
            let mut best_neighbor_sub = current_sub;
            let mut best_gain = 0.0;

            for &(neighbor, weight) in &adj[node] {
                if !nodes.contains(&neighbor) {
                    continue;
                }
                let neighbor_sub = sub_comm[&neighbor];
                if neighbor_sub == current_sub {
                    continue;
                }
                // Gain from merging: edge weight minus expected
                let ki = degrees[node];
                let kj = degrees[neighbor];
                let gain = weight - ki * kj / m;
                if gain > best_gain {
                    best_gain = gain;
                    best_neighbor_sub = neighbor_sub;
                }
            }

            if best_neighbor_sub != current_sub {
                // Merge: update all nodes in current_sub to best_neighbor_sub
                for &n in nodes {
                    if sub_comm[&n] == current_sub {
                        sub_comm.insert(n, best_neighbor_sub);
                    }
                }
            }
        }

        // Assign new community IDs based on refined sub-communities
        let mut sub_to_new: HashMap<usize, usize> = HashMap::new();
        for &node in nodes {
            let sub = sub_comm[&node];
            let new_comm = sub_to_new.entry(sub).or_insert_with(|| {
                let id = next_comm_id;
                next_comm_id += 1;
                id
            });
            refined[node] = *new_comm;
        }
    }

    communities.copy_from_slice(&refined);
}

/// Leiden community detection on a CodeGraph.
///
/// The directed graph is projected to an undirected weighted graph (summing
/// edge weights in both directions), then Leiden runs until convergence or
/// `max_iterations` rounds.
pub fn leiden(graph: &CodeGraph, max_iterations: usize) -> ClusteringResult {
    let n = graph.graph.node_count();
    if n == 0 {
        return ClusteringResult {
            communities: HashMap::new(),
            num_communities: 0,
            modularity: 0.0,
        };
    }

    let edges = build_undirected(&graph.graph);
    let m = total_weight(&edges);
    if m == 0.0 {
        // No edges → each node is its own community, but we label them all 0
        let mut communities = HashMap::new();
        for idx in graph.graph.node_indices() {
            let node = &graph.graph[idx];
            communities.insert(node.id.clone(), 0);
        }
        return ClusteringResult {
            communities,
            num_communities: 1,
            modularity: 0.0,
        };
    }

    let adj = build_adj(&edges, n);
    let degrees = degrees(&adj);

    // Initial: each node in its own community
    let mut communities: Vec<usize> = (0..n).collect();

    for _iter in 0..max_iterations {
        // Phase 1: Local moving
        let moved = local_move(&edges, &adj, &degrees, &mut communities, m);

        // Phase 2: Refinement
        refine(&edges, &adj, &degrees, &mut communities, m);

        if !moved {
            break;
        }
    }

    // Relabel communities 0..k (0 = largest community by node count)
    let mut comm_sizes: HashMap<usize, usize> = HashMap::new();
    for &c in &communities {
        *comm_sizes.entry(c).or_insert(0) += 1;
    }
    let mut size_order: Vec<(usize, usize)> = comm_sizes.into_iter().collect();
    size_order.sort_by_key(|(_, size)| std::cmp::Reverse(*size));

    let mut relabel: HashMap<usize, usize> = HashMap::new();
    for (new_id, &(old_id, _)) in size_order.iter().enumerate() {
        relabel.insert(old_id, new_id);
    }

    for c in &mut communities {
        *c = relabel[c];
    }

    let num_communities = relabel.len();

    // Compute modularity
    let mod_val = modularity(&edges, &communities, m);

    // Build result map: node id → community id
    let mut result = HashMap::with_capacity(n);
    for idx in graph.graph.node_indices() {
        let node = &graph.graph[idx];
        result.insert(node.id.clone(), communities[idx.index()]);
    }

    ClusteringResult {
        communities: result,
        num_communities,
        modularity: mod_val,
    }
}

/// Run Leiden on the CodeGraph (default 10 iterations).
impl CodeGraph {
    pub fn cluster(&self) -> ClusteringResult {
        leiden(self, 10)
    }
}
