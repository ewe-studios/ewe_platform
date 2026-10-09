# Fundamentals 00 — Code knowledge graphs: what & why

Zero-to-expert on *code knowledge graphs* — what they are, why an agent wants
one, and how this feature (F27a) builds and queries one in Rust. Read the rest of
this folder in order; this file is the map.

---

## 1. The problem: three kinds of "find"

An agent searching a codebase asks three different kinds of question:

1. **Semantic** — "where's the code that *handles retries*?" → embeddings/vectors
   (F24/F25). Good at meaning, blind to exact names.
2. **Lexical** — "where's the literal string `RetryPolicy`?" → BM25 (F26). Good at
   exact tokens, blind to structure.
3. **Structural** — "**which file defines** `RetryPolicy`? **what calls** it? what
   does it **inherit**? what's in its **neighborhood**?" → **a graph**.

Vector and keyword search cannot answer the third kind well. "What calls X" is not
a similarity question; it's a *graph traversal*. That's the gap a code knowledge
graph fills.

## 2. What a code knowledge graph is

A directed graph where:

- **Nodes** are code entities: files, modules, structs/enums/traits, functions,
  methods, impls, constants. Each has an id, a label (its name), a kind, and a
  source location (`file:line`).
- **Edges** are relationships: `contains` (file→struct), `imports`, `inherits`/
  `implements`, `calls`, `uses`. Each edge carries **provenance** (below).

Querying that graph — "neighbors of node N within a token budget", "shortest path
from A to B", "all callers of X" — answers structural questions directly, and does
so with **far fewer tokens** than dumping raw files into the model's context
(graphify measured ~71.5× fewer tokens per query). Token efficiency is the whole
point: the agent gets a precise structural answer instead of re-reading files.

## 3. Provenance: EXTRACTED vs INFERRED

Every edge is tagged with a **confidence**:

- **EXTRACTED** — read directly from the syntax tree. `file contains struct` is
  not a guess; the parser saw it. High trust.
- **INFERRED** — deduced by resolution, not literally present at one site. A
  cross-file `calls` edge (`caller()` in `b.rs` calls `helper()` defined in
  `a.rs`) is inferred by matching the call name against the global symbol index.
  Lower trust (weight 0.8), and honestly labelled as such.

(graphify has a third tier, **AMBIGUOUS**, but that's emitted only by the optional
LLM semantic pass — F27c — never by deterministic AST extraction. F27a emits only
EXTRACTED and INFERRED.)

Provenance is not decoration: when the agent acts on an edge, it should know
whether that edge is fact or inference.

## 4. The pipeline (F27a slice)

```
source files ──extract──▶ per-file {nodes, edges, raw_calls}
                              │
                              ▼
                          build ──▶ DiGraph + dedup + cross-file resolve
                              │
                              ▼
                          query: find_entity / callers_of /
                                 neighborhood(budget) / shortest_path
                              │
                              ▼
                          F32 search() tool
```

- **extract** (`rust_walker`) — tree-sitter parses one file; a hand-written Rust
  walker emits nodes, EXTRACTED edges, and a list of *unresolved* calls
  (`raw_calls`) to resolve later (Doc 03, 04).
- **build** (`CodeGraph::build`) — collect all files' nodes/edges into a petgraph
  `DiGraph`, resolve the `raw_calls` into cross-file INFERRED `calls` edges against
  a global label index, then label-dedup (Doc 04).
- **query** — `find_entity`, `callers_of`, budgeted `neighborhood`,
  `shortest_path` (Doc 05, 08).
- **update** — re-extract changed files and rebuild, re-resolving cross-file edges
  globally (Doc 10).

## 5. What's in scope here (the F27a/b/c split)

Per OD-27-1 this feature is deliberately split:

- **F27a (this feature, done):** extraction core + the **Rust** walker + graph
  build + dedup + the query surface + persistence (`to_bytes`/`from_bytes` +
  `GraphStore`) + incremental `update`.
- **F27b (follow-on):** JS/TS + Python walkers (these can use the generic
  `LanguageConfig` walker — Doc 03).
- **F27c (follow-on):** **Leiden** community detection + god-node analysis + the
  optional **LLM** semantic pass.

This folder teaches all of it (Docs 06, 07, 09 cover the F27c concepts so the
picture is whole), but the implemented, tested code today is F27a.

## 6. Where the graph lives

`foundation_vectors::code_graph`. Build (AST extraction) is **native-only** —
tree-sitter grammars are C and don't compile to `wasm32-unknown-unknown` (Doc 02).
**Query** over a prebuilt graph is pure Rust and **wasm-safe**: a native tool
builds `graph.json` (or `to_bytes`), a wasm deployment loads and queries it. The
`GraphStore` trait (Doc 08, 10) is how that artifact is persisted and shipped.

---

**Next:** Doc 01 — parsing & ASTs (how source text becomes a tree you can walk).
