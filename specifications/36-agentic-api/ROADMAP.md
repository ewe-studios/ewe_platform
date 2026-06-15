# Spec 36 — Feature Roadmap (machinery-first ordering)

The features are numbered to build **core machinery first** (each with in-memory trait
implementations + full `{crate}/tests` coverage), then storage backends, then retrieval/RAG
enhancements, then new platform crates. This file is the authoritative index after the 2026-06-15
re-numbering (see `discussion.md` §F for the rationale).

## Phase 0 — Foundation fixes (prerequisites)
- `00-foundation-compact` — vendored getrandom/rand + scru128/time/entropy, per-target.
- `00b-foundation-ai-llama-optional` — optional llama, target-aware gating, generalized provider errors.
- `00c-foundation-ai-wasm-providers` — wasm provider surface (fetch-client work → Phase 4).
- `00d-wasm-target-matrix` — the 4-target matrix + target_os cfg discipline.

## Phase 1 — Core machinery (in-memory impls + full tests)
- `01-message-model` — type substrate (MessageRole, SessionId, ToolCall deps, SessionRecord).
- `02-error-handling` — `AgenticError` taxonomy (early so nothing stubs it).
- `03-agent-stream-contract` — pure `SessionRecord` stream + `AgentProgress`.
- `04-token-accounting-budget` — `TokenLedger` over `Model::costing()`; budget halt.
- `05-serialization-json-arrow` — JSON + arrow-rs columns (promoted fields).
- `06-documentstore-trait-sql-memory` — DocumentStore trait + **in-memory** backend (SQL too).
- `07-memorystore` — latest-`SessionRecord` cache (KV-backed); coordinator owns it + DocumentStore.
- `08-message-api` — append/scan/pubsub/WAL message log.
- `09-toolimpl-registry` — async `ToolImpl` + registry.
- `10-toolshed-shed-metatool` — `ToolShed` + `shed` discovery.
- `11-toolcall-execution-dag` — ToolCallManager execution (staged DAG on valtron).
- `12-model-provider-router` — `RoutableProvider` + fallback routing.
- `13-steering-queues-depends` — PriorityQueue/FollowUpQueue + readiness `Depends`.
- `14-input-output-processors` — input/output pipelines.
- `15-memory-hierarchy` — working/observation/reflection generation.
- `16-context-provider-assembly` — prompt assembly + memory-trigger firing.
- `17-loop-detection` — repetition detection + redirect.
- `18-access-control-budget` — access control + budget enforcement.
- `19-agentic-loop` — the orchestrating loop (ties Phase 1 together).
- `20-agent-session-api` — public `AgentSession` builder/resume; owns MessageApi + MemoryStore.
- `21-testing-strategy` — coverage strategy; `{crate}/tests` layout.

## Phase 2 — Real storage backends
- `22-documentstore-vfs-fjall-index` — `FjallDocumentStore` (NDJSON + fjall index family).
- `23-documentstore-cloudflare` — D1 (ordered) + R2 (large blobs); KV dropped (it's the Memory cache).

## Phase 3 — Retrieval / RAG enhancements (research-gated; paired with `foundation_docs`)
- `24-foundation-vectors-core` — vector types, metrics, flat scan.
- `25-foundation-vectors-ivf-hnsw` — IVF + HNSW.
- `26-foundation-vectors-bm25-hybrid` — BM25 + RRF fusion.
- `27-foundation-vectors-code-graph` — code-graph (Rust-first).
- `28-vectorstore-trait-inmemory` — VectorStore trait + in-memory.
- `29-vectorstore-native-backends` — native vector backends.
- `30-vectorstore-cloudflare-external` — CF/external (use their API if any, else skip).
- `31-embedding-provider` — embedding provider + router; sentence-level chunking.
- `32-search-tools` — `search_context` + `search_file` (fff-search).

## Phase 4 — Platform investments (new crates; see `discussion.md` §B — not yet authored)
- `foundation_http` — fetch-based client (native + wasm), unblocks remote providers on wasm.
- `foundation_wasmtime` — wasmtime host wrapper (WASI harness, future host work).
- `foundation_buildtools` — build-time concerns (EMSDK wiring, target detection, `build.rs` helpers).
- `foundation_docs` — zero-to-hero fundamentals (vectors/ANN, BM25, code-graphs, embeddings, wasm…).

---

### Old → new number map (2026-06-15 re-numbering)
| was | now | feature |
|----|-----|---------|
| 01 | 01 | message-model |
| 30 | 02 | error-handling |
| 02 | 03 | agent-stream-contract |
| 03 | 04 | token-accounting-budget |
| 17 | 05 | serialization-json-arrow |
| 04 | 06 | documentstore-trait-sql-memory |
| 07 | 07 | memorystore |
| 16 | 08 | message-api |
| 20 | 09 | toolimpl-registry |
| 21 | 10 | toolshed-shed-metatool |
| 23 | 11 | toolcall-execution-dag |
| 24 | 12 | model-provider-router |
| 25 | 13 | steering-queues-depends |
| 26 | 14 | input-output-processors |
| 19 | 15 | memory-hierarchy |
| 18 | 16 | context-provider-assembly |
| 28 | 17 | loop-detection |
| 29 | 18 | access-control-budget |
| 27 | 19 | agentic-loop |
| 31 | 20 | agent-session-api |
| 32 | 21 | testing-strategy |
| 05 | 22 | documentstore-vfs-fjall-index |
| 06 | 23 | documentstore-cloudflare |
| 08 | 24 | foundation-vectors-core |
| 09 | 25 | foundation-vectors-ivf-hnsw |
| 10 | 26 | foundation-vectors-bm25-hybrid |
| 11 | 27 | foundation-vectors-code-graph |
| 12 | 28 | vectorstore-trait-inmemory |
| 13 | 29 | vectorstore-native-backends |
| 14 | 30 | vectorstore-cloudflare-external |
| 15 | 31 | embedding-provider |
| 22 | 32 | search-tools |

> `requirements.md` uses an older coarse phase numbering and is out of sync with these dirs; treat
> this ROADMAP as authoritative for feature ordering.
