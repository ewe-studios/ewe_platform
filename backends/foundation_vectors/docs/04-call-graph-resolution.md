# Fundamentals 04 — Call-graph resolution

"What calls X" is the highest-value query and the easiest to get *wrong*. This doc
explains bare vs member calls, the false-positive "god node" trap, deferred
`raw_calls` resolution, and the 3-layer dedup. It maps directly onto
`graph.rs::resolve_cross_file_calls` and `deduplicate_by_label`.

---

## 1. The core problem

When the walker sees `helper()` in `b.rs`, it knows the *name* but not the *target
node id* — `helper` might be defined in `a.rs`, not yet walked. You can't resolve
the call at the call site. So extraction **defers** it: emit a `RawCall` and
resolve all of them later, after every file's nodes exist.

```rust
struct RawCall {
    caller_id: String,     // the function the call is inside
    callee_name: String,   // "helper"
    is_member_call: bool,  // was it `x.log()` ?
    source_file: String, source_line: usize,
}
```

## 2. Bare calls vs member calls

- **Bare call** — `helper()`, `authenticate()`. The callee name is a plausible
  global symbol. These resolve.
- **Member call** — `x.log()`, `self.run()`, `client.send()`. The callee is a
  *method on some receiver*; the bare name (`log`, `run`, `send`) is hopelessly
  common.

The walker flags member calls (`is_member_call = true`) by checking whether the
call's function child is a field/method-access node (`field_expression` in Rust)
rather than a plain identifier.

## 3. The god-node trap (why member calls are excluded from cross-file)

Here's the failure mode that ruins naive call graphs: resolve member calls by
name, and every `.log()` / `.run()` / `.next()` / `.to_string()` in the codebase
points at *every* `log`/`run`/`next` definition. One `log` method ends up with
thousands of incoming edges — a **false "god node"** that's pure noise and makes
traversal useless.

The fix (graphify, verified): **member calls are excluded from cross-file
resolution.** We resolve only **bare** calls against the global index. Member-call
resolution needs real type information (what *is* `x`?), which the AST pass doesn't
have — that's a job for the optional LLM pass (F27c) or future type-aware
resolution, not name matching.

```rust
for call in raw_calls {
    if call.is_member_call { continue; }      // ← the critical guard
    // resolve bare call by normalized name…
}
```

## 4. The label index and resolution

`build` constructs a **label index**: `normalize_label(name) → [node indices]`
(Doc 02b §5). To resolve a bare call:

1. Normalize the callee name → key.
2. Look the key up in the label index → candidate target nodes.
3. For each candidate, add a `calls` edge `caller → callee`, **INFERRED**,
   weight 0.8 (it's a name match across files, not a literal local edge).

```rust
let key = normalize_label(&call.callee_name);
for &tgt in self.label_index.get(&key).unwrap_or(&vec![]) {
    let pair = (call.caller_id.clone(), self.graph[tgt].id.clone());
    if seen_pairs.insert(pair, true).is_some() { continue; }  // dedup
    self.add_edge(&call.caller_id, &tgt_id, calls_edge_inferred_0_8());
}
```

## 5. `seen_call_pairs` dedup

A caller may call the same callee many times; we want **one** `calls` edge, not
one per call site. `seen_pairs: HashSet<(caller_id, callee_id)>` collapses
duplicates. This keeps `callers_of(X)` returning distinct callers, not a multiset.

## 6. The 3-layer dedup (`deduplicate_by_label`)

The same entity can be discovered more than once (re-exports, multiple files
referencing it). Dedup runs in three layers:

1. **Within-file** — `seen_ids` in each `FileExtraction` prevents emitting the
   same node twice from one file.
2. **Between-file** — `add_node` is idempotent on id: adding an existing id is a
   no-op.
3. **Label-normalized merge** — `deduplicate_by_label` groups nodes by
   `normalize_label`, and for groups with the same **kind**, picks a canonical node
   and **rewrites the duplicates' edges onto it**, dropping self-loops. After
   merge, `Parser` discovered from three files is one node with all three files'
   edges.

The kind check matters: a `struct Foo` and a `fn foo` normalize to the same label
but are different kinds — never merge across kinds.

## 7. Cross-file `uses` (the class-level inference)

Beyond calls, graphify infers class-level **`uses`** edges from imports: if
`b.rs`'s class imports `a.rs`'s `Response`, an INFERRED `uses` edge connects them
(`_resolve_cross_file_imports`). This is the second cross-file mechanism (distinct
from `calls`), turning file-level import facts into entity-level relationships. It
runs in the same post-build resolution pass, against the same global index.

## 8. Why resolution is global (and what that means for `update`)

Because cross-file `calls`/`uses` span files, resolution must see **all** files'
nodes at once. That's why `build` resolves after collecting every extraction — and
why incremental `update` (Doc 10) **re-resolves globally** rather than patching one
file's edges. A change in `a.rs` can create or break edges that originate in
`b.rs`.

---

**Next:** Doc 05 — graph theory you need (directed graphs, degree, paths, BFS/DFS).
