# Spec 36 — Feature Roadmap (machinery-first ordering)

The features are numbered to build **core machinery first** (each with in-memory trait
implementations + full `{crate}/tests` coverage), then storage backends, then retrieval/RAG
enhancements, then new platform crates. This file is the authoritative index after the 2026-06-15
re-numbering (see `discussion.md` §F for the rationale).

## Phase 0 — Foundation fixes (prerequisites)
- `00-foundation-compact` **✓ COMPLETE** — vendored getrandom/rand + scru128/time/entropy, per-target.
- `00b-foundation-ai-llama-optional` **✓ COMPLETE** — optional llama, target-aware gating, generalized provider errors.
- `00c-foundation-ai-wasm-providers` **✓ COMPLETE** — wasm provider surface (fetch-client work → Phase 4).
- `00d-wasm-target-matrix` **✓ COMPLETE** — the 4-target matrix + target_os cfg discipline.
- `00e-unified-send-async-traits` **✓ COMPLETE** — collapse the `?Send` split: one `Send` async-trait surface for all
  `foundation_db` store traits + `SendWrapper` adapter on single-threaded wasm (Item #1 / §A1).
- `00f-wasm-fetch-http-client` — wasm `fetch` client backend for `foundation_netio` (consistent client **✓ COMPLETE**
  API native+wasm + SSE over `ReadableStream`); unblocks wasm providers (00c) + external REST (F30).

## Phase 1 — Core machinery (in-memory impls + full tests)
- `01-message-model` **✓ COMPLETE** — type substrate (MessageRole, SessionId, ToolCall deps, SessionRecord).
- `02-error-handling` **✓ COMPLETE** — `AgenticError` taxonomy (early so nothing stubs it).
- `03-agent-stream-contract` **✓ COMPLETE** — pure `SessionRecord` stream + `AgentProgress`.
- `04-token-accounting-budget` **✓ COMPLETE** — `TokenLedger` over `Model::costing()`; budget halt.
- `05-serialization-json-arrow` **✓ COMPLETE** — JSON + arrow-rs columns (promoted fields).
- `06-documentstore-trait-sql-memory` **✓ COMPLETE** — DocumentStore trait + **in-memory** backend (SQL too).
- `07-memorystore` **✓ COMPLETE** — latest-`SessionRecord` cache (KV-backed); coordinator owns it + DocumentStore.
- `08-message-api` **✓ COMPLETE** — append/scan/pubsub/WAL message log.
- `09-toolimpl-registry` **✓ COMPLETE** — async `ToolImpl` + registry.
- `10-toolshed-shed-metatool` **✓ COMPLETE** — `ToolShed` + `shed` discovery.
- `11-toolcall-execution-dag` **✓ COMPLETE** — ToolCallManager execution (staged DAG on valtron).
- `12-model-provider-router` **✓ COMPLETE** — `RoutableProvider` + fallback routing.
- `13-steering-queues-depends` **✓ COMPLETE** — PriorityQueue/FollowUpQueue + readiness `Depends`.
- `14-input-output-processors` **✓ COMPLETE** — input/output pipelines.
- `15-memory-hierarchy` **✓ COMPLETE** — working/observation/reflection generation.
- `16-context-provider-assembly` **✓ COMPLETE** — prompt assembly + memory-trigger firing.
- `17-loop-detection` **✓ COMPLETE** — repetition detection + redirect.
- `18-access-control-budget` **✓ COMPLETE** — access control + budget enforcement.
- `19-agentic-loop` **✓ COMPLETE** — the orchestrating loop (ties Phase 1 together).
- `20-agent-session-api` **✓ COMPLETE** — public `AgentSession` builder/resume; owns MessageApi + MemoryStore.
- `21-testing-strategy` **✓ COMPLETE** — coverage strategy; `{crate}/tests` layout.

## Phase 2 — Real storage backends
- `22-documentstore-vfs-fjall-index` **✓ COMPLETE** — `FjallDocumentStore` (NDJSON + fjall index family).
- `22b-documentstore-sql-async` **✓ COMPLETE** — `AsyncSqlDocumentStore<Q: AsyncQueryStore>`: canonical async
  `AsyncDocumentStore` for every SQL backend (Turso/Libsql/native+wasm D1) + native sync/async
  conformance. Reused by F23 for D1.
- `23-documentstore-cloudflare` **✓ COMPLETE** — D1 (ordered) + R2 (large blobs); KV dropped (it's the Memory cache).

## Phase 3 — Retrieval / RAG enhancements (research-gated; paired with `foundation_docs`)
- `24-foundation-vectors-core` **✓ COMPLETE** — vector types, metrics, flat scan.
- `25-foundation-vectors-ivf-hnsw` **✓ COMPLETE** — IVF + HNSW.
- `26-foundation-vectors-bm25-hybrid` **✓ COMPLETE** — BM25 + RRF fusion.
- `27-foundation-vectors-code-graph` **✓ COMPLETE** — code-graph (Rust-first).
- `28-vectorstore-trait-inmemory` **✓ COMPLETE** — VectorStore trait + in-memory.
- `29-vectorstore-native-backends` **✓ COMPLETE** — native vector backends.
- `30-vectorstore-cloudflare-external` **✓ COMPLETE** — CF/external (use their API if any, else skip).
- `31-embedding-provider` **✓ COMPLETE** — embedding provider + router; sentence-level chunking.
- `32-search-tools` **✓ COMPLETE** — `search_context` + `search_file` (fff-search).

## Platform investments (new crates; scheduled per `discussion.md` §B)
- **`00f-wasm-fetch-http-client` (Phase 0, EARLY — user)** — add a wasm `fetch` client backend to **✓ COMPLETE**
  `foundation_netio` (native client `SimpleHttpClient` already exists; `foundation_http` is the *server*).
  One consistent client API native+wasm + SSE over `ReadableStream`. Unblocks wasm providers (00c) +
  external REST (F30).
- `foundation_buildtools` (with **00d**) — dedicated crate for build-time concerns (EMSDK wiring, target
  detection, `build.rs` helpers); authored when 00d wires the build matrix.
- `foundation_docs` (**Phase 3**) — mdBook fundamentals (vectors/ANN, BM25, code-graphs, embeddings, wasm…);
  per-feature "Fundamentals" sections become chapters.
- `foundation_wasmtime` (**LAST**, user-deferred) — wasmtime host wrapper; 00d's WASI(wasmtime) runner
  lands together with it. Nothing in core machinery depends on it.

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
| 05b | 22b | documentstore-sql-async |
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
