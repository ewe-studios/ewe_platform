# Fundamentals 08 — Querying & token budgeting

The payoff doc. This is the F27a query surface the `search()` tool (F32) calls,
and *why* it compresses tokens so dramatically. Maps onto `graph.rs`.

---

## 1. The query surface

```rust
impl CodeGraph {
    pub fn find_entity(&self, name: &str) -> Vec<&GraphNode>;          // "which file defines X"
    pub fn callers_of(&self, id: &str) -> Vec<&GraphNode>;            // incoming `calls`
    pub fn callees_of(&self, id: &str) -> Vec<&GraphNode>;            // outgoing `calls`
    pub fn neighborhood(&self, id: &str, budget_tokens: usize)
        -> Result<Subgraph, GraphError>;                              // budgeted BFS/DFS
    pub fn shortest_path(&self, a: &str, b: &str) -> Option<Vec<String>>;
    pub fn get_node(&self, id: &str) -> Option<&GraphNode>;
    pub fn nodes_by_file(&self, file: &str) -> Vec<&GraphNode>;       // "what's in this file"
}
```

These are the structural questions an agent actually asks. Each is a small graph
operation returning a small answer.

## 2. `find_entity` — "which file defines X"

Normalizes the query name (Doc 02b §5) and looks it up in the `label_index`,
returning every matching node (there may be several — overloads, re-exports). Each
node carries `source_file:source_line`, so the answer to "where is `RetryPolicy`?"
is "`src/retry.rs:42`" — not the file's contents.

graphify additionally **scores** candidate start nodes against the natural-language
question (`_score_nodes`) before traversing, so `query "what validates tokens"`
starts from the most relevant node, not an arbitrary name match. F27a exposes the
exact-and-partial label match; question-scoring is a thin layer the `search()` tool
can add on top using F26's ranking.

## 3. `callers_of` / `callees_of`

Pure directed-neighbor lookups (Doc 05 §2): follow incoming vs outgoing `calls`
edges. "What calls `helper`?" returns the caller nodes — including the **cross-file
INFERRED** callers resolved in Doc 04. This is the query that vector/keyword search
simply cannot answer.

## 4. `neighborhood(id, budget_tokens)` — the budgeted traversal

The flagship. Starting from a node, explore outward (BFS by default, DFS optional)
and **accumulate nodes until a token budget is hit**, then stop. Returns a
`Subgraph { nodes, edges, token_estimate }`.

```rust
pub fn neighborhood(&self, id, budget_tokens) -> Result<Subgraph, GraphError> {
    let start = self.id_to_index.get(id).ok_or(NodeNotFound)?;
    // BFS rings outward; estimate tokens per node added; stop at budget.
}
```

Why a budget? Because the consumer is a model with a finite context window. Instead
of "give me everything reachable" (which could be the whole repo), you say "give me
the most relevant ~2000 tokens of structural context around this symbol". BFS
ensures those tokens are spent on the *closest* (most relevant) nodes first
(Doc 05 §4).

## 5. `shortest_path(a, b)`

BFS from `a` to `b`, returning the node-id path or `None`. Answers "how does the
request handler reach the database?" with the actual chain, not prose.

## 6. Why this compresses tokens (~71.5×)

The naive way to answer "what calls `auth_check` and what does it touch" is to dump
several files into the model and let it read. That's thousands of tokens, most
irrelevant. The graph answers the same question with:

- a handful of **node summaries** (`name`, `kind`, `file:line`) — tens of tokens
  each,
- the **edges** among them — a few tokens each,
- bounded by a **budget** so it never overruns.

graphify measured ~**71.5× fewer tokens per query** versus reading raw files. The
mechanism is simple: a graph stores *relationships* explicitly, so you retrieve the
exact relationships asked for instead of re-deriving them by reading code. Token
budgeting then guarantees the answer fits the window with the most relevant context
first.

## 7. wasm-safe

Every method here is pure petgraph traversal over deserialized data — **no
tree-sitter, no I/O**. So a wasm deployment that loaded a prebuilt graph
(`from_bytes`, Doc 02 §6 / Doc 10) runs the entire query surface. Build native,
query anywhere.

---

**Next:** Doc 09 — the optional LLM semantic pass (F27c).
