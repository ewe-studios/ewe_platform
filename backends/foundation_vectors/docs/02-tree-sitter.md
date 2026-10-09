# Fundamentals 02 — tree-sitter

The one external dependency we keep. Parsing 25 languages correctly is decades of
work; tree-sitter already did it. Everything *else* — the walker, the graph, the
query — is ours. This doc covers tree-sitter's model, the Rust bindings, and the
crucial wasm caveat.

---

## 1. What tree-sitter is

tree-sitter is an incremental parsing library plus a family of language grammars.
You give it source bytes and a `Language`; it gives you a concrete syntax tree you
can navigate by node type and field name. It's designed for editors (syntax
highlighting, structural selection), which is why it has two properties we
exploit: **error recovery** and **incremental re-parsing**.

## 2. The object model

- **`Language`** — a compiled grammar (e.g. `tree_sitter_rust::LANGUAGE`). Newer
  grammar crates expose a `LanguageFn` you call to get the `Language`; older ones
  expose a `language()` function. (Doc 03's `LanguageConfig` abstracts this.)
- **`Parser`** — holds a `Language`; `parser.parse(source, old_tree)` returns a
  `Tree`.
- **`Tree`** — the parse result; `tree.root_node()` is the top `Node`.
- **`Node`** — a typed span of the source. Key methods:
  - `node.kind()` → the grammar node type as a string (`"function_item"`).
  - `node.child_by_field_name("name")` → the named child for a grammar *field*.
  - `node.named_children(&mut cursor)` → iterate meaningful children.
  - `node.utf8_text(source)` → the exact source slice for this node.
  - `node.start_position().row` → the line (for `source_location`).
- **`TreeCursor`** — a fast, allocation-free way to walk the tree depth-first.
- **`Query`** — an S-expression pattern language to match node shapes (we mostly
  walk with a cursor rather than query, but queries are available).

## 3. GLR parsing and error recovery

tree-sitter uses a **GLR** parser (generalized LR), which can handle the
ambiguities real grammars have. The payoff we care about is **error recovery**:
when the input doesn't match, tree-sitter doesn't bail — it produces `ERROR` and
`MISSING` nodes around the bad region and continues. A file that's broken in one
function still yields every other entity. Extraction therefore degrades
gracefully (Doc 01 §5).

## 4. Incremental parsing

If you keep the old `Tree` and pass it back into `parse(source, Some(&old_tree))`,
tree-sitter re-parses only the changed region — fast enough for per-keystroke
editor use. We don't need per-keystroke parsing, but the same idea informs our
**incremental `update`** (Doc 10): re-extract only changed *files*, not the whole
repo.

## 5. The Rust bindings

```rust
use tree_sitter::Parser;

let mut parser = Parser::new();
parser.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
let tree = parser.parse(source_bytes, None).unwrap();
let root = tree.root_node();
// walk root with a TreeCursor, dispatch on node.kind()
```

Each language is a separate grammar crate (`tree_sitter_rust`,
`tree_sitter_python`, …). Per OD-27-4 these are **feature-gated** so a consumer
pulls only the grammars it needs — F27a only needs `tree_sitter_rust`, behind the
`code-graph` feature.

## 6. The wasm caveat (OD-27-5) — read this twice

tree-sitter grammars are **C code**, compiled by a `cc` build script. **C
grammars do NOT compile to `wasm32-unknown-unknown`.** So:

- **Build (extraction) is native-only.** You cannot run the tree-sitter walker in
  a `wasm32-unknown-unknown` Worker.
- **Query is pure Rust → wasm-safe.** Once a graph is built and serialized
  (`to_bytes`/`graph.json`), querying it needs no parser at all — just petgraph
  traversal over deserialized nodes/edges.

The resolution (OD-27-5): a **native tool builds the graph**; a **wasm deployment
loads and queries** the prebuilt artifact. That's why the crate target-gates the
walker off wasm but keeps `CodeGraph`'s query + serde paths target-agnostic.

(There *is* a separate "web-tree-sitter" toolchain that ships grammars as `.wasm`
artifacts loaded at runtime — a different mechanism from `cc`-compiled native
grammars. We don't use it; the native-build / wasm-query split is simpler and
matches how the graph is actually consumed.)

---

**Next:** Doc 02b — node id stability (why ids must survive across machines).
