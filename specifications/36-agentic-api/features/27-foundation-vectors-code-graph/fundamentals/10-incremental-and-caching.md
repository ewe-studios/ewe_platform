# Fundamentals 10 — Incremental update, caching & persistence

The last doc: how the graph stays fresh without re-parsing the world, and how it's
persisted and shipped to a wasm query deployment. Maps onto `graph.rs::update` /
`remove_file` and `serial.rs` (`to_bytes`/`from_bytes` + `GraphStore`).

---

## 1. The cost we're avoiding

Re-extracting and rebuilding an entire repo on every edit is wasteful — most files
didn't change. Two ideas keep it cheap: **content-hash caching** (skip re-parsing
unchanged files) and **incremental update** (rebuild from cached extractions,
re-resolving only what must be global).

## 2. Content-hash caching

graphify hashes each file's contents and caches its `FileExtraction`
(`load_cached`/`save_cached`). On a run, a file whose hash matches the cache is
**not re-parsed** — its cached nodes/edges/raw_calls are reused. Because extraction
is deterministic (Doc 01 §4), a cached extraction is *exactly* what re-parsing
would produce, so this is a pure speedup with no correctness cost.

The unit of caching is the **`FileExtraction`** — the per-file structural output.
That's also the unit `CodeGraph` retains in memory (`extractions` map) to support
update.

## 3. Incremental `update`

```rust
pub fn update(&mut self, changed: Vec<FileExtraction>) {
    for e in changed { self.extractions.insert(e.file_path.clone(), e); }
    self.rebuild();          // re-resolve cross-file globally
}
pub fn remove_file(&mut self, file_path: &str) {
    self.extractions.remove(file_path);
    self.rebuild();
}
```

The caller re-extracts the changed files (cheap — only those files hit the parser;
the rest stay cached), hands the fresh `FileExtraction`s to `update`, and the graph
**replaces those entries and rebuilds**.

## 4. Why rebuild, not patch (the global re-resolution scope — OD-27-7)

It's tempting to surgically remove a changed file's nodes/edges and splice in the
new ones. **Don't.** Cross-file `calls`/`uses` edges *originate in other files*
(Doc 04 §8): editing `a.rs` (rename `helper`→`assist`) must delete a `calls` edge
that lives on `b.rs`'s function, and *re-resolve* whether `b.rs`'s call now matches
anything. You cannot know that from `a.rs` alone. So `update` **re-resolves
globally** — exactly graphify's behavior (OD-27-7). Rebuilding from the retained
per-file extractions is both simpler and *correct*; the expensive part (parsing)
was already skipped by caching, so rebuild is cheap.

`remove_file` is the same story for deletions: drop the entry, re-resolve globally
so dangling cross-file edges to the deleted file disappear.

## 5. Persistence: `to_bytes` / `from_bytes` + `GraphStore` (OD-27-6)

Graph metadata is **not columnar** — it's nodes and edges with string fields — so
it serializes as **plain serde/JSON**, not arrow. (Arrow is reserved for columnar
sidecars like node-embedding matrices, if ever added.) The graph exposes:

```rust
impl CodeGraph {
    pub fn to_bytes(&self) -> Result<Vec<u8>, GraphError>;   // JSON bytes (graph.json parity)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, GraphError>;
}
```

and a backend-agnostic store trait:

```rust
pub trait GraphStore {
    fn save(&mut self, key: &str, bytes: &[u8]) -> Result<(), GraphError>;
    fn load(&self, key: &str) -> Result<Option<Vec<u8>>, GraphError>;
    fn save_graph(&mut self, key, graph) -> …;   // convenience
    fn load_graph(&self, key) -> Result<Option<CodeGraph>, GraphError>;
}
```

`InMemoryGraphStore` is the default (tests, small repos); a fjall /
`DocumentStore`-backed impl serves large repos. The graph never depends on which —
it just hands over bytes.

## 6. The build-native / query-anywhere split

This is where everything ties together (Doc 02 §6, Doc 08 §7):

1. A **native tool** extracts (tree-sitter) and builds the graph, then
   `store.save_graph("repo", &graph)`.
2. A **wasm deployment** does `store.load_graph("repo")` → `from_bytes` → and runs
   the full **query** surface. No parser, no I/O — pure petgraph.

A graph loaded `from_bytes` is **query-only**: it has nodes and edges but no
retained `FileExtraction`s, so it can't `update` (rebuild would have nothing to
rebuild from). That's intentional and matches the architecture — **building and
updating are native; loading and querying are anywhere.**

---

That's the whole pipeline: parse (01–03) → resolve (04) → graph (05) → [cluster +
analyze: 06–07, F27c] → query under budget (08) → [optional LLM: 09, F27c] →
persist & update incrementally (10). F27a ships the deterministic spine; F27b adds
languages; F27c adds clustering, analysis, and the semantic pass.
