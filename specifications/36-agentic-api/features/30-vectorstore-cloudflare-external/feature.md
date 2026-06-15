---
feature: "VectorStore: Cloudflare (D1/KV/Vectorize) + external (Pinecone/Chroma/TurboPuffer)"
description: "AsyncVectorStore backends for serverless + managed vector DBs — Cloudflare D1/KV fetch-then-search (and Vectorize if available), plus native HTTP clients for Pinecone/Chroma/TurboPuffer behind the same trait"
status: "pending"
priority: "low"
depends_on: ["28-vectorstore-trait-inmemory"]
estimated_effort: "large"
created: 2026-06-14
last_updated: 2026-06-14
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 11
  total: 11
  completion_percentage: 0%
---

# Feature 30: VectorStore — Cloudflare + external backends

**TODO**: I have reached the limits of my knowledge, lets do web research and select the best answers for these for the different platforms we wish to support, then add foundation_docs to teach me from zero to hero on all these topics in detail and depth. Also if wasm is not possible in some areas that is ok.
If CF does not work with vectorstore, then that is fine, if it has an API for it then lets use that and not waste time trying to build one in unless its really viable else dont waste effort.

> **Review status (2026-06-14) — broken references + a decision conflict:**
> 1. **F28 never actually defines `AsyncVectorStore`** (it's only in F28's review note, not its trait
>    block) — this dependency is a phantom until **F28's WHAT/HOW/Done-When add the
>    `#[async_trait(?Send)]` signature**, or F30 owns defining it.
> 2. **External vector DBs — RESOLVED (user, 2026-06-15):** they ARE supported, **behind the existing
>    `VectorStore`/`AsyncVectorStore` trait** (F28) — Decision 07's blanket rejection is reversed (a
>    decisions write-back must amend Decision 07 to record this). **Scope now:** **TurboPuffer is a
>    REQUIRED implementation** — a thin native HTTP adapter over our own HTTP client against its REST
>    API (no third-party crate needed). **Pinecone + Chroma are DEFERRED** — trait-ready, not
>    implemented now. So 14b ships TurboPuffer.
> 3. **No CF Vectorize runtime binding exists** (only deployment DTOs in `foundation_deployment`) —
>    OD-30-1 resolves to **"absent — greenfield `bindgen/cf/vectorize.rs`"**; size accordingly.
> 4. **CF D1/KV inherit F23's hard limits**: KV `list` can't paginate past 1000 yet (binding fix is a
>    prerequisite — recall is *truncated*, not just best-effort); D1 has no batch/`exec()`.
> 5. **Provider impedance mismatch**: Chroma *collections* (own dim/metric/lifecycle) vs Pinecone/
>    TurboPuffer *namespaces* (partitions). Map F28's `namespace: String` per provider; define index/
>    collection provisioning + who creates/deletes (OD-30-7). Dimension is server-side/async-fail for
>    external providers (deviates from F28's insert-time check).
> 6. **Commit the 14a (CF, wasm) / 14b (external HTTP, native) split** (OD-30-2). Client-side flat
>    search over fetched D1/KV vectors needs a **hard cap + truncation policy** (OD-30-8). Credentials
>    via `SessionAccessProvider` makes **F08 a dependency** (declare it) (OD-30-9).
> (Note: the Chroma exploration is at `@formulas/.../src.VectorDB/src.Chroma`, **outside this repo** —
> reference its public REST API, don't assume in-repo source.)

> Implements Decision 07's Cloudflare backends + Decision 03b's external managed vector DBs, behind
> the F28 **`AsyncVectorStore`** trait. Completes the VectorStore backend set (in-memory F28, native
> F29, CF+external F30). Lowest priority — the in-memory + native backends cover the core use cases;
> these are for serverless / large-scale managed deployments.

## WHY: Problem Statement

Serverless (CF Workers) and large managed deployments need vector storage beyond local. CF
**Vectorize** (native ANN) or D1/KV (fetch-then-`foundation_vectors`) cover wasm/edge; Pinecone/
Chroma/TurboPuffer cover managed scale. All behind one trait so the agentic layer is backend-agnostic.

## WHAT: Solution

All implement **`AsyncVectorStore`** (`#[async_trait(?Send)]`, F28) — these are network/Promise-based.

### Cloudflare

- **Vectorize** (if available in the Workers binding): native ANN `query()`; namespace via metadata
  filter. Preferred CF path. **Research:** is the Vectorize binding exposed in our worker stack?
- **D1 / KV fallback:** store vectors as BLOB (D1) / JSON (KV); **fetch namespace-scoped set +
  client-side `foundation_vectors::flat_top_k`**. Bounded by collection size (CF KV 1000-key cap +
  eventual consistency — inherits F23's caveats). Best-effort for large sets.

### External managed (native HTTP) — scoped per user ruling (2026-06-15)

The `VectorStore`/`AsyncVectorStore` trait (F28) is the well-structured seam; external providers are
adapters behind it. **Scope now:**
- **TurboPuffer — REQUIRED.** A thin native `AsyncVectorStore` adapter over our own HTTP client
  against the **TurboPuffer REST API** (`upsert`/`query`/`delete` namespaces). No third-party crate —
  we own the adapter. Chosen as the supported external for its simple REST model + serverless pricing.
- **Pinecone + Chroma — DEFERRED.** The trait makes them droppable later; no adapter is written now.
- Namespace = the provider's native namespace concept; dimension = provider index config (server-side,
  async-fail — deviates from F28's local insert-time check). Native-only (HTTP transport; 00c). API
  keys via config / `SessionAccessProvider` (F08), never committed.

### Shared

- Same `insert/query(namespace)/delete` semantics; dimension enforced; namespace isolation.
- Result re-ranking uses `foundation_vectors` only when the backend lacks native ANN (D1/KV).
- Feature-gated per backend (pull only what a deployment needs).

## Architecture

```mermaid
graph TD
    A[AsyncVectorStore F28] --> VEC[CF Vectorize: native ANN]
    A --> DKV[CF D1/KV: fetch + flat_top_k best-effort]
    A --> PIN[Pinecone HTTP]
    A --> CHR[Chroma HTTP]
    A --> TP[TurboPuffer HTTP]
    DKV -->|client-side| FV[foundation_vectors flat]
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: managed/serverless vector DBs (CF Vectorize, Pinecone, Chroma,
TurboPuffer) — their data models, namespaces, ANN indexes; fetch-then-search vs native ANN; the cost
of client-side search at scale (CF KV caps, round-trips); async `?Send` HTTP on wasm vs native;
provider REST API adapters; when to relax ordering/consistency guarantees. (Task — see list.)

## HOW: Implementation Steps

1. Research: CF Vectorize binding availability; the **TurboPuffer REST API** (auth, namespaces,
   upsert/query/delete shapes, dimension/metric config).
2. CF D1/KV `AsyncVectorStore` (fetch + flat fallback) — reuse F23 D1/KV bindings + caveats.
3. CF Vectorize backend (if available) — native ANN.
4. **TurboPuffer `AsyncVectorStore` REST adapter** (native, feature-gated) — required.
   Pinecone/Chroma deferred (trait-ready, not implemented).
5. Dimension + namespace each; mock-binding/mock-HTTP tests + opt-in live integration.

## Open Decisions

- **OD-30-1 — CF Vectorize availability:** is the binding in our worker stack? If yes, prefer it over
  D1/KV fetch-then-search. Research.
- **OD-30-2 — external scope:** **Resolved (user, 2026-06-15) → TurboPuffer REQUIRED** (native REST
  adapter over our HTTP client; no crate); **Pinecone + Chroma deferred**. Split into
  **14a (CF: Vectorize + D1/KV, wasm)** and **14b (TurboPuffer REST adapter, native)**.
- **OD-30-3 — D1/KV best-effort:** explicitly document KV's eventual-consistency + 1000-cap mean
  KV-vector is best-effort; D1 fetch is bounded by row count. (Inherits F23.)
- **OD-30-4 — async transport on wasm:** external HTTP providers are native-only (wasm has no HTTP —
  00c). CF backends are wasm. State the split.
- **OD-30-5 — credentials:** provider API keys via config/`SessionAccessProvider` (F08), never committed.

## Target Files

- `backends/foundation_db/src/wasm/{cf_vectorize, d1_vector, kv_vector}_store.rs` (wasm)
- `backends/foundation_db/src/core/backends/{pinecone, chroma, turbopuffer}_vector_store.rs` (native HTTP)
- feature-gated per backend

## Tests

```bash
cargo test -p foundation_db -- vector_store::{cf,external}   # mock bindings/HTTP
```

## Verification

```bash
cargo build -p foundation_db --target wasm32-unknown-unknown --features <cf-vector>
cargo build -p foundation_db --features <external-vector>
cargo clippy -p foundation_db --all-features -- -D warnings
cargo test  -p foundation_db -- vector_store
```

## Done When

- CF (Vectorize and/or D1/KV) + **TurboPuffer (REST adapter)** implement `AsyncVectorStore` with
  dimension + namespace; CF builds wasm, external builds native; parity tests (mock) pass.
- Best-effort caveats documented; fundamentals authored. OD-30-1..5 resolved (incl. 14a/14b split).
