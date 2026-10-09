# Feature 16: Port walrus transformation capabilities (DEFERRED)

**Status:** DEFERRED — intention recorded; revisit on trigger below.
**Library target (if undeferred):** `foundation_codegen` (transformation IR)
**Derive/proc-macro:** `foundation_macros` (walrus uses `walrus-macro`) — per [[feedback_macros_location]]
**Upstream:** [`walrus`](https://github.com/rustwasm/walrus) v0.23.3, MIT/Apache-2.0, © Nick Fitzgerald
**Decisions:** 031 (owned WASM infrastructure)
**Relates to:** F15 (wasmbin — the owned default; preferred), F14 (wasm-bindgen boundary)

---

## Goal (deferred)

Bring walrus's WebAssembly **transformation** capabilities — a mutable IR with automatic
reference bookkeeping — into our owned codegen stack, for cases where heavy structural rewrites
(rewiring call graphs, removing functions/types with relocation, GC of unused entities) make
wasmbin's faithful 1:1 model painful.

---

## Complexity assessment (why deferred)

| Aspect | Finding |
|---|---|
| Size | ~12,547 LOC |
| Self-contained? | **No** — wraps `wasmparser` (decode) + `wasm-encoder` (encode); the IR sits between |
| Heavy deps | `id-arena`, `wasm-encoder`, `wasmparser`, `gimli` (DWARF), `rayon`, `walrus-macro` |
| Hardest modules | `ir/mod.rs` (1361), `module/functions/local_function/{mod 1618, emit 923, context}`, `function_builder.rs` (513) |
| Reference management | `tombstone_arena.rs` (deletion via tombstones) + `passes/{used,gc}.rs` (used-set + GC) |
| Debug info | `module/debug/*` (~1k LOC) preserves DWARF via `gimli` |
| Overlap with F15 | parse/serialize fully overlaps wasmbin; only the mutable IR + auto-relocation is unique |

**Conclusion:** a full port is a large, dependency-pulling effort that mostly duplicates wasmbin,
for a transformation model we don't currently need. Defer. If a real need arises, prefer
cherry-picking the specific mechanism (reference cleanup) onto wasmbin (see F15) over a full port.

---

## What we'd port, if undeferred

- The **Id-arena entity model** (functions, types, globals, memories, tables, elements, data) with
  stable ids and cross-reference resolution.
- The **instruction IR** + `FunctionBuilder`/`emit` for building/rewriting function bodies.
- **`tombstone_arena` + `passes::used`/`gc`** — deletion + dead-entity collection with reference
  fixups (the auto-cleanup capability).
- Optionally **DWARF preservation** (gimli) for debuggable rewrites.

## Trigger to un-defer

Revisit when we need **heavy structural rewrites** that wasmbin makes painful — e.g. removing or
reordering functions/types with automatic relocation of all references, call-graph rewiring, or
GC of unused entities across a real module. Until then, F15 (wasmbin) covers our additive/local
edits, and F15's planned reference auto-cleanup covers targeted deletions.

## Attribution (if undeferred — MIT/Apache-2.0)

Same rules as F15: vendor `LICENSE-MIT` + `LICENSE-APACHE`, NOTICE crediting *walrus, © Nick
Fitzgerald, https://github.com/rustwasm/walrus*, per-file credit headers, README links, upstream
version recorded, modifications stated. Workspace is Apache-2.0 — compatible.

## Success criteria (when undeferred)

- [ ] Owned mutable IR in `foundation_codegen` round-trips real modules.
- [ ] Delete/relocate entities with automatic reference fixups (tombstone + GC).
- [ ] `FunctionBuilder`/emit rewrites function bodies.
- [ ] Full MIT/Apache-2.0 attribution.
- [ ] Decision documented on walrus IR vs extending wasmbin — avoid maintaining two full models.
