---
feature: "foundation_vectors: crate core + distance metrics + flat scan"
description: "New foundation_vectors crate — Vector type, cosine/L2/dot distance metrics, and a brute-force flat-scan top-k search. Pure Rust, WASM-safe; the algorithmic foundation the VectorStore backends (F28-14) call"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: 2026-06-14
last_updated: 2026-06-14
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0%
---

# Feature 24: foundation_vectors — core + distance + flat scan

> **Review status (2026-06-14):** folded — (1) **`f32` is not `Ord`**, so the top-k heap needs an
> `OrderedScore(f32)` newtype with a manual `Ord` via `f32::total_cmp` (NaN sorts as least), which
> also implements the NaN guard; (2) **`f32::sqrt` is `std`-only** → the `no_std` goal needs `libm`
> or a squared-norm/normalized-dot formulation (revised OD-24-5); (3) cosine needs a **zero-vector
> policy** (NaN→never-selected); (4) `VectorMatch`/`DistanceMetric` are **owned here and re-exported
> by `foundation_db`** (Decision 07 double-defines them); (5) add `foundation_vectors` to
> `[workspace.dependencies]` (F28 needs it), `serde = { workspace = true }`, `[lints] workspace =
> true`, `edition.workspace = true` (2021). See OD-24-6/7/8.

**TODO**: Why does any of these need to leak there, if its a problem, move it all into foundation_vector and let it own it all fully

> First feature of the new **`foundation_vectors`** crate (verified: does not exist; `backends/*`
> workspace glob auto-includes it). Implements Decision 07's "algorithms owned by us" — starting with
> the core types, distance metrics, and flat (brute-force) scan. IVF/HNSW (F25), BM25/hybrid (F26),
> and code-graph (F27) build on this. Pure Rust + WASM-safe (no native-only deps).

## WHY: Problem Statement

Decision 07: vector search algorithms live in `foundation_vectors` (owned by us) so behavior is
consistent across every backend, there's always a fallback when a DB lacks native vector search, and
it works in WASM. Nothing exists yet (no `VectorStore`, no `cosine`, no `hnsw` anywhere). This
feature lays the foundation: the vector representation, the distance metrics, and the simplest
correct search (flat scan) — enough for the in-memory `VectorStore` (F28) and as the fallback path
for all backends.

## WHAT: Solution

### Crate setup

```
backends/foundation_vectors/
├── Cargo.toml      # pure Rust; no_std-friendly where possible; NO rayon (Item #16)
└── src/
    ├── lib.rs
    ├── metric.rs   # DistanceMetric + the math
    ├── vector.rs   # Vector + dimension handling
    ├── flat.rs     # flat-scan top-k (sequential)
    └── store.rs    # VectorStore trait + VectorEntry + in-memory backend (moved from foundation_db, Item #16)
```

WASM-safe: no `fjall`/`memmap2`/`rayon` in the default build. **No rayon at all (Item #16):** rayon
saturates all CPU cores via work-stealing, starving valtron's thread pool. Instead:
- **Multi-threaded targets (native + emscripten):** chunk the scan and spawn to valtron's background
  thread queue. Workers pick up chunks alongside other valtron tasks — no core starvation.
- **Single-threaded wasm (unknown-unknown, wasip1/p2):** sequential scan (no threads exist).
- **One concurrency substrate — valtron — everywhere.**

### Vector + metrics

```rust
/// A dense embedding. Dimension is carried for validation (cross-model mixing is a bug — F23).
#[derive(Debug, Clone, PartialEq)]
pub struct Vector { pub data: Vec<f32> }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric { Cosine, L2, Dot }

impl DistanceMetric {
    /// Similarity score where HIGHER = more similar (so top-k is a max-heap for all metrics).
    /// Cosine → cosine similarity; Dot → dot product; L2 → negative squared L2 (so higher=closer).
    pub fn score(&self, a: &[f32], b: &[f32]) -> f32;  // debug-asserts a.len()==b.len()
}
```

- **Cosine** is primary for text embeddings; precompute norms where possible (OD-24-1: store
  normalized vectors so cosine == dot, avoiding per-query norm).
- All metrics return a **higher-is-better** score so `flat`/IVF/HNSW share one top-k convention.
- Dimension mismatch is a `debug_assert!` + a checked `try_score` returning `Err` (OD-24-2).

### Flat scan (brute force)

```rust
pub struct VectorMatch { pub id: String, pub score: f32 }

/// Brute-force top-k over an iterator of (id, vector). O(n·d). Correct for any n; the baseline +
/// the universal fallback. Uses a bounded min-heap of size k.
pub fn flat_top_k<'a>(
    query: &[f32],
    entries: impl Iterator<Item = (&'a str, &'a [f32])>,
    k: usize,
    metric: DistanceMetric,
) -> Vec<VectorMatch>;
```

- Bounded min-heap of size k keyed by `OrderedScore` — **NOT** raw `f32` (`BinaryHeap<T>` needs
  `T: Ord`; `f32` is only `PartialOrd`). `OrderedScore(f32)` impls `Ord` via `f32::total_cmp`
  (available in `core`, so no_std-safe) with **NaN ordered as least** (so NaN scores are never
  selected — the NaN guard lives here). Memory O(k), one pass. `k == 0` → empty `Vec`.
- **No rayon (Item #16).** Parallel scan uses valtron background threads on multi-threaded targets;
  sequential on single-threaded wasm. Identical results.
- This is what the in-memory `VectorStore` (now in this crate, Item #16) uses, and the fallback for
  backends without native ANN (F29/F30).

### Numerics & determinism

- f32 throughout (embeddings are f32; matches `ModelOutput::Embedding.values: Vec<f32>`).
- NaN guard: treat NaN scores as -inf (never selected). Ties broken by id for determinism (OD-24-3).
- No SIMD intrinsics in the portable path (WASM); an optional native SIMD feature is a later perf
  follow-up (OD-24-4).

## Architecture

```mermaid
graph TD
    Q[query vector] --> M[DistanceMetric.score]
    E[entries id,vec] --> F[flat_top_k bounded heap]
    M --> F
    F --> R[Vec VectorMatch higher=better]
    F28[in-memory VectorStore] --> F
    F29[native backends fallback] --> F
```

## HOW: Implementation Steps

1. Create the crate (`backends/foundation_vectors`, pure Rust, no rayon).
2. `Vector` + `DistanceMetric` + `score`/`try_score` with the higher-is-better convention.
3. Normalized-vector option for cosine (OD-24-1).
4. `flat_top_k` with bounded min-heap; NaN/tie handling. Sequential by default.
5. Parallel scan via valtron background threads (multi-threaded targets); sequential on single-threaded wasm.
6. `VectorStore` trait + `VectorEntry` + in-memory backend (moved from F28/foundation_db, Item #16).
7. Tests: metric correctness (known vectors), top-k correctness vs naive sort, k>n, empty, NaN,
   tie-determinism, sequential==parallel; wasm build.

## Open Decisions

- **OD-24-1 — normalized storage:** store L2-normalized vectors so cosine == dot (faster queries) vs
  normalize per-query. Rec: normalized storage (the store normalizes on insert).
- **OD-24-2 — dimension mismatch:** `debug_assert!` + checked `try_score(-> Result)`; the VectorStore
  (F28) enforces a fixed dimension at insert (Decision 07). Rec: both.
- **OD-24-3 — tie-breaking:** by id ascending for deterministic results. Rec: yes.
- **OD-24-4 — SIMD:** portable scalar now; native SIMD feature later. Rec: defer.
      Whats the block for SIMD ?

- **OD-24-5 — no_std (revised):** `BinaryHeap`/`Vec` are in `alloc` ✓, and `f32::total_cmp` is in
  `core` ✓, **but `f32::sqrt` is `std`-only** — cosine's norm needs it. Options: (a) add `libm` for
  `sqrt` in no_std, (b) rank on squared-norm forms avoiding sqrt, (c) restrict no_std to the
  normalized-dot path (store normalized → cosine==dot, no sqrt at query). Rec: (a) `libm` (clean) or
  (c). Don't claim no_std without resolving sqrt.
      My knowledge is lacking but i remember DOOM had a CPU friendly way to do sqrt, would not that resolve issues here in no std land ?

- **OD-24-6 (mandatory) — f32 ordering:** `OrderedScore(f32)` newtype, manual `Ord` via
  `total_cmp`, NaN = least. The heap key + NaN guard in one place. (Not `ordered-float` dep — keep it
  ours/no_std.)

- **OD-24-7 — cosine zero/empty-vector policy:** `score(Cosine,…)` when `‖a‖` or `‖b‖` is 0 (→ 0/0
  NaN) → return a score that sorts as least (never selected); query must be normalized when OD-24-1
  is on. Define explicitly.
        Ya, my knowledge lacks, we need fundamental documents explain vector store adn their algorithmn to make me go zero to genius. Research the web, select the best option here

- **OD-24-8 — type ownership: RESOLVED (user, 2026-06-15; Item #16) → `foundation_vectors` owns it
  all.** `VectorStore` trait, `VectorEntry`, `VectorMetadata`, `VectorStoreConfig`, `VectorMatch`,
  `DistanceMetric` — all live in `foundation_vectors`. `foundation_db` re-exports for convenience.
  The in-memory backend (formerly F28's scope) also lives here. F28 becomes "VectorStore in-memory
  backend" within `foundation_vectors`, not a `foundation_db` deliverable.

- **OD-24-9 — borrowed-iterator vs streaming backends:** `flat_top_k`'s `(&str,&[f32])` iterator
  fits in-memory (F28) but not the D1/KV fetch-then-search path (F30, deserializes on the fly). Either
  narrow the "fallback for all backends" claim to eager/in-memory backends, or add an owned/streaming
  `flat_top_k` variant. Rec: add an owned variant when F30 needs it.
        Ya, explain more, i dont understand, lets talk on it

## Target Files

- `backends/foundation_vectors/` (new crate): `Cargo.toml`, `src/{lib,metric,vector,flat}.rs`
- root `Cargo.toml` — picked up by `backends/*` glob (verify)

## Tests

```bash
cargo test -p foundation_vectors
cargo build -p foundation_vectors --target wasm32-unknown-unknown
```

## Verification

```bash
cargo build -p foundation_vectors
cargo build -p foundation_vectors --target wasm32-unknown-unknown
cargo clippy -p foundation_vectors -- -D warnings
cargo test  -p foundation_vectors
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: embeddings & vector spaces; **distance metrics** (cosine, L2, dot —
geometry, when each applies, normalization making cosine==dot); top-k selection & bounded heaps;
**`f32` is not `Ord`** (`total_cmp`, NaN handling); zero-vector/division-by-zero; `no_std`+`alloc`
& `f32::sqrt`/`libm`; SIMD basics. (Task — see list.)

## Done When

- `foundation_vectors` builds (native + wasm, no rayon); `DistanceMetric` (cosine/L2/dot) and
  `flat_top_k` are correct (tested vs naive) with deterministic ties and NaN safety.
- `VectorStore` trait + in-memory backend live here (Item #16); `foundation_db` re-exports.
- Parallel scan uses valtron background threads (multi-threaded), sequential on single-threaded wasm.
- The higher-is-better score convention is uniform (ready for IVF/HNSW in F25).
- OD-24-1..8 resolved.
