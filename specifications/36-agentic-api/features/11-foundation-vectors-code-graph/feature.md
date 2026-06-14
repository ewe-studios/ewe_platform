---
feature: "foundation_vectors: code-graph (graphify-derived, Rust-native)"
description: "A Rust-native code knowledge-graph in foundation_vectors — deterministic tree-sitter AST extraction (file/class/function nodes; imports/contains/inherits/calls/uses edges with EXTRACTED/INFERRED/AMBIGUOUS confidence), raw_calls cross-file resolution, rationale extraction, Leiden clustering, god-node analysis, and BFS/DFS query — powering the agentic search() tool"
status: "pending"
priority: "medium"
depends_on: ["08-foundation-vectors-core"]
estimated_effort: "large"
created: 2026-06-14
last_updated: 2026-06-14
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 16
  total: 16
  completion_percentage: 0%
---

# Feature 11: foundation_vectors — code-graph (graphify-derived)

> **Fidelity review (2026-06-14) — important corrections to the source study below:**
> 1. **"One generic walker drives all languages" is FALSE.** Only ~12 languages use
>    `_extract_generic`+`LanguageConfig`; ~11 have **bespoke extractors** — including **Rust
>    (`extract_rust`, extract.py:2180) and Go (`extract_go`, :1963)**, the priority languages. So the
>    "additive LanguageConfig" premise does NOT hold for Rust/Go; they are hand-written walkers. The
>    16-task estimate is far too low — extraction parity for the priority langs alone is most of it.
> 2. **`LanguageConfig` shape is wrong below:** the real dataclass (extract.py:88) holds `ts_module:
>    str` + `ts_language_fn: str` (loaded by name), plus omitted fields (`static_prop_types`,
>    `helper_fn_names`, `container_bind_methods`, `event_listener_properties`, `*_fallback_child_types`,
>    `function_boundary_types`).
> 3. **Provenance:** the AST pass emits ONLY `EXTRACTED` + `INFERRED` (+ numeric `confidence_score`,
>    e.g. 0.8). **`AMBIGUOUS` is LLM-pass only** (llm.py/analyze.py), not AST.
> 4. **Relations** actually emitted include `defines/includes/instantiates/method/binds_method/
>    bound_to/listened_by/references_constant/uses_component/uses_static_prop`; my `References` is an
>    LLM-pass relation, not AST.
> 5. **Cross-file is two mechanisms:** Python-only class-level INFERRED **`uses`**
>    (`_resolve_cross_file_imports`) vs **all-language** INFERRED **`calls`** (0.8) with a *double*
>    member-exclusion (`callee_node_type ∈ _MEMBER_CALL_NODE_TYPES` AND `is_member_call`). I conflated
>    them.
> 6. **Rationale:** graphify creates a **separate node** (`file_type:"rationale"`) + `rationale_for`
>    edge (and excludes them from the cross-file label index). Using an *attribute* instead is a
>    deliberate **divergence** (legitimate, but own its consequences), not graphify behavior.
> 7. **Leiden = `graspologic`** (no drop-in pure-Rust equivalent — real risk; OD-11-3 must resolve,
>    not soft-pedal). Cluster graph is **undirected** (Leiden needs it) even though the query graph is
>    **directed**.
> 8. **Query** does start-node **scoring against the question** (`_score_nodes`) then depth-2 BFS/DFS —
>    `find_entity`/`neighborhood` must include that to match `query "<q>"`.
> 9. **tree-sitter C grammars do NOT compile to `wasm32-unknown-unknown`** via `cc`; the wasm story
>    needs the separate web-tree-sitter `.wasm` grammar toolchain (OD-11-5 + fundamentals 02).
> 10. **`_file_stem` is parent-qualified** (extract.py:51) to avoid same-name-file id collisions — the
>     `make_id(stem,name)` model must include this.
>
> **MVP slice for the agentic use case** ("which file has this entity / what calls it"):
> `extract (Rust+Python+JS/TS+Go) → build+dedup → find_entity / callers_of (incl. cross-file INFERRED
> calls) / neighborhood(budget, with query scoring)`. Cluster/analyze/god-nodes/LLM-pass are a LATER
> sub-feature → confirms the **11a (extraction+query MVP) / 11b (graph build+cluster+analyze) / 11c
> (LLM semantic pass)** split.

> Implements Decision 07 + Decision 14's code-graph ("graph search e.g. code graphs we've generated
> which can tell us which file has this given block of code or entity"). It is a **Rust-native
> reimplementation of graphify's deterministic extraction + graph pipeline**, derived from the
> Python source at `@formulas/src.rust/src.AI/src.Graphify/graphify/graphify/`. The agentic
> `search()` tool (F22) queries this graph for structural questions ("which file defines `X`", "what
> calls `Y`", "what's in this module"). tree-sitter is the one justified external dependency (parsing
> 25 languages is not worth reimplementing); **everything else — the walker, graph, dedup, Leiden
> clustering, analysis, query — is ours**.

## WHY: Problem Statement

Vector (F08/F09) + BM25 (F10) answer "semantically/lexically similar". They do **not** answer
**structural** questions: "which file defines `auth_check`?", "what calls it?", "what does this
class inherit?", "what's the neighborhood of this symbol?". graphify proves a code knowledge-graph
gives this with **71.5× fewer tokens per query** vs reading raw files, with a full provenance audit
trail (every edge tagged EXTRACTED/INFERRED/AMBIGUOUS). The agent needs the same: a queryable,
provenance-tracked graph of the codebase, owned in `foundation_vectors`, reused by F18/F22's search.

## Source study — what graphify actually does (verified against the Python)

The graphify pipeline is **7 stages of pure functions**: `detect(root) → extract(paths) [AST + LLM]
→ build(graph) → cluster(Leiden) → analyze(god nodes) → report/export → query`. The two extraction
passes:

- **Pass 1 — deterministic AST (the Rust-portable core, `extract.py`, 3611 lines).** No LLM. Covers
  25 languages via one generic walker + per-language config.
- **Pass 3 — LLM semantic (`llm.py` + `skill.md`).** Optional; extracts edges AST can't (cross-file
  calls, shared data, doc concepts). Code-only corpora **skip it**.

### AST extraction model (replicate exactly)

**`LanguageConfig`** (one generic walker drives all languages; `extract.py:88`):

```rust
pub struct LanguageConfig {
    pub grammar: tree_sitter::Language,        // e.g. tree_sitter_python::LANGUAGE
    pub class_types: &'static [&'static str],     // {"class_definition"} (py), {"struct_item","enum_item","trait_item"} (rs)
    pub function_types: &'static [&'static str],  // {"function_definition"} (py), {"function_item"} (rs)
    pub import_types: &'static [&'static str],    // {"import_statement","import_from_statement"} (py), {"use_declaration"} (rs)
    pub call_types: &'static [&'static str],      // {"call"} (py), {"call_expression"} (rs)
    pub call_accessor_node_types: &'static [&'static str], // {"attribute"} (py), {"field_expression"} (rs)
    pub name_field: &'static str,              // "name"
    pub body_field: &'static str,              // "body"
    pub call_function_field: &'static str,     // "function"
    pub call_accessor_field: &'static str,     // "attribute"
    pub import_handler: Option<ImportHandlerFn>,       // per-language import edge emitter
    pub resolve_function_name_fn: Option<NameResolverFn>, // C/C++ declarator unwrapping
    pub extra_walk_fn: Option<ExtraWalkFn>,    // JS arrow fns, C# namespaces, Swift extras
}
```

**Node schema** (`add_node`, `extract.py:763`):
```rust
struct GraphNode { id: String, label: String, file_type: String /* "code" */,
                   source_file: String, source_location: String /* "L{line}" */ }
```
- `id = make_id(stem, name)` → lowercase, `[a-z0-9_]` only, no dots/slashes (`_make_id`, `:37`).
  File node id = `make_id(path)`; later remapped absolute→**relative** path for cross-machine
  stability (`:3444`).

**Edge schema** (`add_edge`, `:774`):
```rust
struct GraphEdge { source: String, target: String, relation: Relation,
                   confidence: Confidence, source_file: String, source_location: String, weight: f32 }
enum Confidence { Extracted, Inferred, Ambiguous }   // the provenance audit trail
enum Relation { Imports, ImportsFrom, Contains, Inherits, Implements, Calls, Uses, References, RationaleFor }
```

**The walk** (`_extract_generic.walk`, `:789`), per node type:
1. **import** → `import_handler` emits `imports`/`imports_from` edges (per-language: Python dotted/
   relative resolution, JS string-path normalize + `.js`→`.ts`, Java/C/C#/Kotlin/Scala/PHP variants).
2. **class** → `contains` edge from file; per-language **inheritance**: Python `superclasses`, Swift
   `inheritance_specifier`, C# `base_list`, Java `superclass`/interfaces → `inherits`/`implements`.
3. **function** → node + `contains`; body queued for call extraction.
4. **call resolution** (`walk_calls`, `:1072`) — the subtle part:
   - Build `label_to_nid` map (normalized label → node id, `:1049`).
   - For each call, extract callee name. **Member calls** (`obj.log()`) are marked
     `is_member_call=true` and **excluded from cross-file resolution** (common names like `log`/`run`
     create thousands of false-positive "god node" edges, `:17`).
   - Bare function calls (`authenticate()`) resolve against `label_to_nid` → `calls` edge; `seen_call_pairs`
     dedups `(caller,callee)` (`:1055`). Unresolved → `raw_calls` list for cross-file resolution.
5. **rationale** (`_extract_python_rationale`, `:1375`): comment prefixes `# NOTE:`, `# IMPORTANT:`,
   `# HACK:`, `# WHY:`, `# RATIONALE:`, `# TODO:`, `# FIXME:` + docstrings → a `rationale` **attribute**
   on the relevant node (updated guidance: not a separate node).

**Two-pass `extract()`** (`:3340`): (1) per-file structural extraction with **content-hash caching**
(`load_cached`/`save_cached` — unchanged files skip re-parse); (2) **cross-file import resolution**
turns file-level imports into class-level **INFERRED** `uses` edges (`DigestAuth --uses--> Response`).
Dispatch by suffix over **35+ extensions / 25 languages** (`_DISPATCH`, `:3376`).

### LLM semantic pass (optional — uses the agentic provider, not a separate API)

graphify calls Claude `claude-sonnet-4-6` (temp 0) or Kimi `kimi-k2.6`, `max_tokens=8192`, 20-file
chunks (`llm.py:14-45`). **System prompt** (verbatim, `llm.py:31`): "extract a knowledge graph
fragment… Output ONLY valid JSON… EXTRACTED/INFERRED/AMBIGUOUS… node id `{stem}_{entity}`…". The
subagent prompt (`skill.md:296`) adds: code files → semantic edges AST can't find (don't re-extract
imports); `calls` edges source=caller target=callee (never reversed); rationale as an attribute.

**In our port:** the LLM pass is **optional** and routed through the agentic loop's own
`ModelProviderRouter` (F24) / memory model — not a bespoke API client. The deterministic AST pass is
the default and the only thing needed for "which file has this code".

### Graph build, cluster, analyze, query

- **build** (`build.py`): directed graph; **3-layer dedup** — within-file `seen_ids`, between-file
  idempotent `add_node`, and **label-normalized merge** (`deduplicate_by_label`, lowercase-alnum key,
  rewrites edges, drops self-loops). Use `petgraph` or our own adjacency (OD-11-2).
- **cluster** (`cluster.py`): **Leiden** community detection (Louvain fallback), stable community ids
  (0=largest), recursive split of large communities (`_MIN_SPLIT_SIZE=10`), cohesion score
  (intra/maximum edges). Own it in Rust (OD-11-3).
- **analyze** (`analyze.py`): **god nodes** (highest-degree hubs), **surprising connections**
  (cross-community edges), suggested questions.
- **query** (`__main__.py`): **BFS/DFS** traversal from a node with a **token budget** (default 2000);
  **shortest path** between two nodes; **explain** (node + neighbors). This is the search() backend.

### Command surface (graphify CLI → our API methods)

`detect`, `extract`, `build`, `cluster`, `analyze`, `query "<q>" [--dfs --budget N]`,
`path A B`, `explain X`, `update <path>` (incremental re-extract, **no LLM**), `merge-graphs`,
`benchmark` (token-reduction measure). These become `CodeGraph` methods, not a CLI.

## WHAT: Solution (Rust)

```rust
// foundation_vectors::code_graph, native (tree-sitter), target-gated off wasm for *build*.
pub struct CodeGraph { /* petgraph DiGraph<GraphNode, GraphEdge> + label_index + community map */ }

impl CodeGraph {
    pub fn extract_file(path: &Path) -> Result<FileExtraction, GraphError>;   // AST, cached
    pub fn build(extractions: Vec<FileExtraction>) -> CodeGraph;              // dedup + cross-file resolve
    pub fn update(&mut self, changed: &[PathBuf]);                            // incremental, no LLM
    pub fn cluster(&mut self);                                                // Leiden
    pub fn god_nodes(&self, k: usize) -> Vec<&GraphNode>;
    // Query surface used by F22 search():
    pub fn find_entity(&self, name: &str) -> Vec<&GraphNode>;                 // "which file defines X"
    pub fn callers_of(&self, id: &str) -> Vec<&GraphNode>;
    pub fn neighborhood(&self, id: &str, budget_tokens: usize) -> Subgraph;   // BFS/DFS budgeted
    pub fn shortest_path(&self, a: &str, b: &str) -> Option<Vec<String>>;
    pub fn to_json(&self) / from_json(...);                                   // persist (graph.json parity)
}
```

`Relation`/`Confidence`/`GraphNode`/`GraphEdge` live in `foundation_vectors`. The graph integrates
with hybrid search (F10): a `search()` can fuse vector + BM25 + **graph proximity**.

### Platform

- **Build (AST extraction)** is **native-only** (tree-sitter grammars are C; target-gate off wasm).
- **Query** over a pre-built `graph.json` is **pure Rust → WASM-safe**. So a wasm deployment can
  ship a prebuilt graph and query it, just not rebuild it. (OD-11-5.)

## Architecture

```mermaid
flowchart TD
    F[code files] --> AST["tree-sitter walk (LanguageConfig)"]
    AST -->|nodes+edges+raw_calls| RES[cross-file resolve → INFERRED uses]
    RES --> B[build: 3-layer dedup, DiGraph]
    B --> CL[Leiden cluster + god nodes]
    B --> Q["query: find_entity / callers / neighborhood(budget) / path"]
    Q --> S[F22 search() tool]
    AST -.optional.-> LLM[LLM semantic pass via ProviderRouter]
    LLM --> B
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `features/11-foundation-vectors-code-graph/fundamentals/` (numbered deep docs, house style of
`specifications/completed/03-wasm-friendly-sync-primitives/fundamentals/`) covering — so an engineer
goes from zero to expert in code knowledge-graphs:

- `00-overview.md` — code knowledge graphs: what/why, the 71.5× token-compression result, provenance
  (EXTRACTED/INFERRED/AMBIGUOUS), where graph search beats vector/keyword.
- `01-parsing-and-asts.md` — lexing/parsing, concrete vs abstract syntax trees, grammars, error
  recovery; why structural extraction is deterministic and free.
- `02-tree-sitter.md` — tree-sitter architecture (GLR, **error recovery**, incremental parsing), the
  node/field/cursor model, `Language`/`Parser`/`Query`, grammar crates (`LanguageFn` vs `language()`),
  the Rust bindings, **and the C-grammar vs `wasm32` reality** (native `cc` grammars do NOT build for
  `wasm32-unknown-unknown` — wasm needs web-tree-sitter `.wasm` grammar artifacts).
- `02b-node-id-stability.md` — id collision handling: parent-qualified `_file_stem`, absolute→relative
  remap, label normalization, why ids must be stable across machines.
- `03-language-config-and-generic-walking.md` — the one-walker-many-languages pattern; node-type
  taxonomies across 25 languages; per-language import/inheritance handling.
- `04-call-graph-resolution.md` — bare vs member calls, the false-positive "god node" problem,
  deferred `raw_calls` resolution, label→id maps, dedup, cross-file INFERRED edges.
- `05-graph-theory.md` — directed graphs, adjacency/degree, paths, BFS/DFS, centrality.
- `06-community-detection.md` — modularity, Louvain, **Leiden** (refinement, guarantees), cohesion,
  recursive splitting.
- `07-graph-analysis.md` — god nodes (centrality), surprising connections (cross-community), question
  suggestion.
- `08-querying-and-token-budgeting.md` — budgeted traversal, shortest path, neighborhood extraction,
  why this compresses tokens.
- `09-llm-semantic-extraction.md` — the optional Pass-3 model + prompts, confidence taxonomy,
  chunking, when code-only corpora skip it.
- `10-incremental-and-caching.md` — content-hash caching, incremental update, global re-resolution
  scope, id remapping for cross-machine stability.

(Task: these docs are part of feature completion — see task list.)

## HOW: Implementation Steps

1. Crate module `code_graph`; tree-sitter + per-language grammar deps (native, feature-gated).
2. `LanguageConfig` + the generic `walk` (imports/classes/functions); node/edge/relation/confidence.
3. Per-language configs + import handlers + inheritance for the priority languages (Rust, Python, JS/TS,
   Go, Java first; then the rest of the 25). OD-11-1.
4. `walk_calls` + `label_to_nid` + member-call exclusion + `raw_calls` + `seen_call_pairs` dedup.
5. Rationale extraction (comment prefixes + docstrings → node attribute).
6. Two-pass `extract()` + content-hash cache + absolute→relative id remap + cross-file INFERRED `uses`.
7. `build` with 3-layer dedup (`deduplicate_by_label`); `update` incremental (no LLM).
8. Leiden clustering (+ Louvain fallback), cohesion, god nodes, surprising connections.
9. Query: `find_entity`, `callers_of`, budgeted BFS/DFS `neighborhood`, `shortest_path`.
10. `to_json`/`from_json` (graph.json parity) for persistence + wasm-side query.
11. Optional LLM semantic pass via the agentic provider (F24) — code-only corpora skip it.
12. Tests: per-language extraction fixtures (vs known nodes/edges), call resolution + member-call
    exclusion, cross-file uses, dedup, Leiden determinism, query/budget, json round-trip; benchmark
    token-reduction vs raw-file reading.

## Open Decisions

- **OD-11-1 — language coverage scope:** all 25 now vs a priority set (Rust/Python/JS-TS/Go/Java)
  first, rest incrementally. Rec: priority set first; `LanguageConfig` makes the rest additive. **This
  is realistically several features — likely split (11a extraction core + priority langs, 11b graph
  build/cluster/analyze, 11c query + LLM pass). Flag for the user.**
- **OD-11-2 — graph lib:** `petgraph` vs own adjacency. Rec: `petgraph` (mature, pure-Rust, wasm-ok).
- **OD-11-3 — Leiden in Rust:** vetted crate vs implement. Rec: evaluate a pure-Rust leiden/louvain
  crate; Decision 07 says own algorithms, but clustering is well-defined — wrap if wasm-safe.
- **OD-11-4 — tree-sitter grammar deps:** 25 grammar crates is heavy. Gate each language behind a
  feature so consumers pull only what they need. Rec: per-language features, a `code-graph-common` set.
- **OD-11-5 — wasm:** build is native (tree-sitter C); query is portable. Confirm the split (prebuilt
  graph.json queried on wasm). tree-sitter does have a wasm build — future option to also build on wasm.
- **OD-11-6 — graph storage:** in-memory + json now; persist to fjall/DocumentStore (F05/F13) for
  large repos? Rec: json now; fjall-backed later.
- **OD-11-7 — incremental correctness:** `update` re-extracts changed files but cross-file `uses`/call
  resolution may span unchanged files — define the re-resolution scope (graphify re-resolves globally).

## Target Files

- `backends/foundation_vectors/src/code_graph/{mod,config,walk,resolve,rationale,build,cluster,analyze,query,langs/*}.rs` (new)
- `backends/foundation_vectors/Cargo.toml` — tree-sitter + per-language grammars (native, feature-gated)
- builds on F08 (`VectorMatch` for graph-proximity fusion), integrates with F10 (hybrid), F22 (search tool)

## Tests

```bash
cargo test -p foundation_vectors -- code_graph
cargo build -p foundation_vectors --features code-graph
```

## Verification

```bash
cargo build -p foundation_vectors --features code-graph
cargo build -p foundation_vectors --target wasm32-unknown-unknown   # query path only (no grammars)
cargo clippy -p foundation_vectors --features code-graph -- -D warnings
cargo test  -p foundation_vectors -- code_graph
```

## Done When

- Deterministic AST extraction reproduces graphify's node/edge/relation/confidence model for the
  priority languages (verified against fixtures), incl. member-call exclusion + cross-file INFERRED `uses`.
- `build`/`update`/`cluster`/`god_nodes` + the query surface (`find_entity`/`callers_of`/budgeted
  `neighborhood`/`shortest_path`) work; `graph.json` round-trips.
- Native build extracts; wasm builds the query path over a prebuilt graph.
- OD-11-1..7 resolved (incl. the likely 11a/b/c split).
