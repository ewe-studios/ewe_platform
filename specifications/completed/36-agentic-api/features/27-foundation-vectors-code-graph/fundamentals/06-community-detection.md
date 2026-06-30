# Fundamentals 06 — Community detection (Leiden)

> **Scope:** clustering is the **F27c follow-on**, not F27a. This doc is the
> concept primer so the picture is complete and F27c starts from understanding,
> not a blank page.

Finding the natural *modules* of a codebase — groups of nodes more connected to
each other than to the rest. This is how the graph offers "what subsystem is this
in?" and powers god-node / surprising-connection analysis (Doc 07).

---

## 1. What a "community" is

A **community** (cluster) is a set of nodes densely connected internally and
sparsely connected to other sets. In a code graph, communities tend to fall out
along real architectural lines — the auth subsystem, the parser, the storage
layer — *without* anyone labelling them. That's the appeal: emergent module
structure from pure connectivity.

## 2. Modularity — the score being optimized

**Modularity (Q)** measures how much better a given partition is than random:
"fraction of edges inside communities" minus "the fraction you'd expect if edges
were rewired randomly". High Q = communities that capture real structure.
Community detection = search for the partition that maximizes Q. (Modularity is
defined on an **undirected** graph — Doc 05 §6 — so clustering uses the undirected
projection.)

## 3. Louvain — the well-known greedy method

**Louvain** optimizes modularity greedily in two repeating phases:

1. **Local moving** — each node joins whichever neighbor's community most increases
   Q, repeated until no move helps.
2. **Aggregation** — collapse each community into a super-node and recurse.

It's fast and gives good Q, but has a known flaw: it can produce **badly connected
or even internally disconnected communities** (a community that local-moving never
re-checked for connectivity).

## 4. Leiden — Louvain done right

**Leiden** adds a **refinement** phase that guarantees every community is
**internally connected** and pushes toward a better optimum:

1. Local moving (as Louvain).
2. **Refinement** — within each community, re-partition so sub-communities are
   well-connected; only well-connected sets survive.
3. Aggregate using the refined partition; recurse.

Leiden's guarantees (connected communities, no degeneracy) are why graphify uses
it (via Python's `graspologic`) and why it's the F27c target. **Risk (OD-27-3):**
there is no drop-in pure-Rust `graspologic` equivalent — F27c must either find a
wasm-safe pure-Rust Leiden/Louvain crate or implement the algorithm. Louvain is the
fallback if Leiden proves too costly.

## 5. Stable community ids and recursive splitting

For the output to be *useful* across runs, graphify:

- **Sorts communities by size and numbers them** (`0` = largest) so ids are stable
  and meaningful.
- **Recursively splits** communities larger than `_MIN_SPLIT_SIZE` (10): a giant
  "everything" cluster is uninformative, so big communities are re-clustered into
  sub-communities.

## 6. Cohesion

Each community gets a **cohesion** score: roughly intra-community edges over the
maximum possible — how tight the cluster actually is. Cohesion lets the agent
distinguish a crisp subsystem (high cohesion) from a loose grab-bag (low), and
feeds the analysis in Doc 07.

## 7. What clustering enables (forward pointer)

Once nodes have community ids:

- **God nodes** (Doc 07) — the highest-degree hubs, often spanning communities.
- **Surprising connections** — edges that cross community boundaries, often the
  interesting architectural seams.
- **"What's in this subsystem?"** — a community is a ready-made answer.

---

**Next:** Doc 07 — graph analysis (god nodes, surprising connections) — also F27c.
