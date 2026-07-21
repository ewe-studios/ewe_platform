# Spec 60 — Feature Index & Roadmap

The agentic stack is proven correct and complete: full offline test coverage,
the standard tool surface (read/write/edit/bash + memory/delegate) implemented
on the VFS, clean test-feature gating, and the carryover items from spec 36
(completed/36-agentic-api) that were left as slots or deferred.

## North star

**100% coverage of every critical path; the standard tools exist and work on a
swappable VFS base; external services and live models are cleanly gated and
runnable on demand with a tiny model.**

## Test tiers (recap)

| Tier | Weights | Gate | For |
|------|---------|------|-----|
| offline | committed tiny fixtures (Llama+Gemma2 safetensors+GGUF), mocks, synthetic | **default** | everything not needing a service/download |
| live-model | SmolLM2 (pulled on demand) | `live-model-tests` | real model semantics via candle/llama.cpp |
| external-service | none (uses the network) | `external-service-tests` | OpenRouter/OpenAI/Anthropic/HF-download/cloud HTTP |

## Feature roadmap

### Phase A — Coverage & test infrastructure

| # | Feature | Status |
|---|---------|--------|
| F01 | Coverage harness + test matrix (was S0/S7) | **DONE** — cargo-llvm-cov; 116/119 matrix; agentic ~86% |
| F02 | Test-feature gating — `live-model-tests` + `external-service-tests` split | **DONE** — both features exist; OpenRouter tests self-skip without key |
| F03 | Live-model SmolLM harness — pull-on-demand, cached, offline-skip | **DONE** — HF GGUF + candle provider download/inference tests gated `external-service-tests`, shared cache (SmolLM pulled once), self-skip without `HF_TOKEN`; offline parse/describe run by default |
| F04 | Drive coverage to 100% of critical logic | **DONE** — foundation_ai 79% lines, no file <70%; 1181 tests green |

### Phase B — Standard agentic tools (VFS-backed)

The `ToolShed` declares `read` / `edit` / `write` / `shell` slots but ships **no
implementations** — the agent literally cannot touch the filesystem or run a
command. These implement them as `ToolImpl`s over the VFS
(`foundation_nativeapis` `VfsFileSystem`) so the FS base is swappable.

| # | Feature | Status |
|---|---------|--------|
| F05 | `read` tool — read a file (range/whole) via VFS | **DONE** — `ReadTool<F>` over `AsyncVfsFileSystem`; 1-indexed offset/limit; 4 tests |
| F06 | `write` tool — create/overwrite a file via VFS | **DONE** — `WriteTool<F>`; create/overwrite, returns byte count; 2 tests |
| F07 | `edit` tool — targeted string replace in a file via VFS | **DONE** — `EditTool<F>`; unique-match unless `replace_all`; absent/non-unique error; 4 tests |
| F08 | `bash` tool — run a shell command (native), captured stdout/stderr/exit; target-gated | **DONE** — `BashTool` native `sh -c` with timeout; wasm returns unsupported; 4 tests |
| F09 | Tool defaults wiring — register read/write/edit/bash in the `ToolShed` + `ToolCallManager` defaults, gated by an injected VFS/exec capability | **DONE** — `register_file_tools`/`register_shell_tool`; `build_toolshed()` fills slots by category; 2 tests |

### Phase C — Provider & quality carryover (earlier spec 60)

| # | Feature | Status |
|---|---------|--------|
| F10 | Candle multi-architecture (Llama+Gemma2 done; Qwen/Mistral/Phi3 when fixtures exist) | Partial |
| F11 | Generation quality — conditional BOS + phantom-tool fix (was S9) | **DONE** (docs/fixes/007) |
| F12 | llama.cpp logging routed through tracing, silent by default (was B) | **DONE** |
| F13 | Known warts — cache race, dead field, ledger-record, abort, preflight compression, panic containment | **DONE** |

### Phase D — Spec 36 carryover (left as slots / deferred)

Surfaced from `specifications/completed/36-agentic-api` — declared but never
implemented, or explicitly deferred.

| # | Feature | Source | Status |
|---|---------|--------|--------|
| F14 | `memory` tool — `MemoryTool` ToolShed slot has NO impl | 36/F10, F15 | **DONE** — memory_add/remove/replace over MemoryHierarchy working memory; 7 tests |
| F15 | `delegate` tool — `DelegationTool` slot has NO impl | 36/F14 | Not started |
| F16 | Real semantic recall — `search_context` Semantic mode is KEYWORD matching, not embeddings; wire EmbeddingProvider+VectorStore | 36/F16, F31 | **DONE** — cosine embedding recall in ContextProvider (keyword fallback when no embedder); AgentSessionBuilder::with_embedder; 2 tests prove feline→cat semantic match |
| F17 | Graph search — `SearchMode::Graph` returns EMPTY (`F27 deferred`); wire or honestly-disable | 36/F27 | Not started |
| F18 | fff file search — `foundation_ai` uses the basic `InCodeVfsSearcher`, NOT the real fff engine (`vfs-search-fff` not enabled); route `search_file` through fff on native | 36/F32, F33 | Not started |

## Working agreements

- No `#[traced_test]` — `#[valtron_test]` handles tracing.
- Tests use foundation capabilities only; elevate reusable glue into the crate
  (e.g. `PreloadedProvider`, `MockTool.with_category`).
- Tools use the VFS so the FS base swaps; native shell is target-gated.
- Every feature: finish 100% (impl + tests + coverage) before the next; mark
  complete in this index and commit.

## Immediate TODO queue (what we're mid-stream on)

1. ~~**F02**: `external-service-tests` feature + move OpenRouter/cloud/HF-download tests under it.~~ **DONE**
2. ~~**F03**: SmolLM pull-on-demand harness (HF GGUF + candle providers).~~ **DONE**
3. ~~**F05–F09**: read/write/edit/bash tools on the VFS + wire defaults.~~ **DONE**
4. **F04**: run the full feature list under llvm-cov to measure coverage, then
   close remaining low-coverage files — `models/generator.rs`,
   `openai_responses_provider.rs`, `harness/agents.rs` tail,
   `toolbox/llama_server_harness.rs`. (In progress — coverage run underway.)
5. **F14–F17**: spec-36 carryover — memory tool, delegate tool, semantic recall,
   graph search, fff file search.
