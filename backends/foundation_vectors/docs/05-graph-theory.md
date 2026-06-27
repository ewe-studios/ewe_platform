# Fundamentals 05 — Graph theory you actually need

Just enough graph theory to understand the query surface and the clustering doc.
No proofs; concepts mapped to what the code does with `petgraph`.

---

## 1. Directed graphs

Our graph is a **directed graph (digraph)**: edges have a direction. `caller
--calls--> callee` is not the same as `callee --calls--> caller`. Direction is the
whole point — "what calls X" follows edges *into* X; "what does X call" follows
edges *out of* X.

- **Node / vertex** — a code entity.
- **Edge / arc** — a directed relationship with a `Relation` + `Confidence`.
- **`petgraph::DiGraph<GraphNode, GraphEdge>`** — the concrete structure. Nodes and
  edges carry our payload types. We keep side maps `id_to_index` (id → NodeIndex)
  and `label_index` (normalized label → indices) for O(1) lookup, because petgraph
  indexes by opaque `NodeIndex`, not by our string ids.

## 2. Degree, in-degree, out-degree

- **out-degree(N)** — number of edges leaving N. For a function, roughly "how many
  things it calls/uses".
- **in-degree(N)** — number of edges entering N. High in-degree = many things
  depend on N. This is the signal behind **god nodes** (Doc 07): the highest-degree
  hubs.
- `callers_of(X)` = neighbors along **incoming** `calls` edges; `callees_of(X)` =
  neighbors along **outgoing** `calls` edges. In petgraph:
  `graph.edges_directed(idx, Direction::Incoming)`.

## 3. Paths and reachability

A **path** is a sequence of nodes connected by edges. "Is B reachable from A?" and
"what's the *shortest* path A→B?" answer questions like "does this handler
eventually reach the database layer, and how?".

- `shortest_path(a, b)` runs a breadth-first search from `a` and returns the first
  (hence shortest in edge count) path to `b`, or `None`. BFS gives shortest paths
  in unweighted graphs because it explores by distance.

## 4. BFS vs DFS

Two ways to explore outward from a start node:

- **Breadth-first search (BFS)** — visit all distance-1 neighbors, then distance-2,
  etc. Explores in *concentric rings*. Best when you want "the closest, most
  relevant context first" — which is exactly what budgeted `neighborhood` wants
  (Doc 08): fill the token budget with nearby nodes before distant ones.
- **Depth-first search (DFS)** — follow one path as deep as it goes, then
  backtrack. Best for "trace this call chain to its end". `neighborhood` supports a
  `--dfs` flavor for chain-following.

graphify's query uses **depth-2 BFS/DFS** from scored start nodes — close enough to
be relevant, bounded enough to stay cheap.

## 5. Subgraphs

A **subgraph** is a node subset plus the edges among them. Every query returns a
subgraph (`Subgraph { nodes, edges, token_estimate }`), not the whole graph — the
answer to "neighborhood of X" is a little graph the agent can read in a few hundred
tokens.

## 6. Directed vs undirected (a clustering note)

The *query* graph is **directed** — direction carries meaning. But **community
detection** (Doc 06, F27c) needs an **undirected** view: Leiden/Louvain optimize a
symmetric modularity score. So clustering builds an undirected projection (collapse
edge direction, sum weights) while queries keep the directed graph. Same nodes, two
views, for two different jobs.

## 7. Weights

Edges carry a `weight: f32`. EXTRACTED structural edges are full-weight; INFERRED
cross-file edges are 0.8 (Doc 04). Weights feed two things: clustering cohesion
(Doc 06) and any future ranked traversal (prefer high-confidence edges when the
budget is tight).

---

**Next:** Doc 06 — community detection (Leiden) — a concept primer for the F27c
follow-on.
