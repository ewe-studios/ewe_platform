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
| F04 | Drive coverage to 100% of critical logic | **DONE** — foundation_ai **88.03% lines / 86.74% regions**, no file <70%; **1909 tests, 0 failures** (first fully green full-feature run). Getting to green cost 4 real bug fixes, not more tests — see [findings.md](findings.md) |

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
| F15 | `delegate` tool → **agentic delegator** (background LLM delegation) | 36/F14 | **DONE** — `MultiCommands` agent tool schedules a child LLM turn as a valtron task on the existing router; handle + `abort()`. First cut reverted (blocking `run_turn` anti-pattern). Deviation: per-call `persist` override not implemented — durability comes from the D/M type parameters. See features/F15-agentic-delegator.md |
| F16 | Real semantic recall — `search_context` Semantic mode is KEYWORD matching, not embeddings; wire EmbeddingProvider+VectorStore | 36/F16, F31 | **DONE** — cosine embedding recall in ContextProvider (keyword fallback when no embedder); AgentSessionBuilder::with_embedder; 2 tests prove feline→cat semantic match |
| F17 | Graph search — `SearchMode::Graph` returns EMPTY (`F27 deferred`); wire or honestly-disable | 36/F27 | **DONE** — honestly disabled: no session knowledge graph exists (code_graph indexes CODE), so Graph warns + falls back to hybrid recall instead of silently empty; 1 test |
| F18 | fff file search — `foundation_ai` uses the basic `InCodeVfsSearcher`, NOT the real fff engine (`vfs-search-fff` not enabled); route `search_file` through fff on native | 36/F32, F33 | **DONE** — enabled vfs-search-fff (native); SearchFileTool::native uses native_vfs_searcher cascade (fff→CLI→in-code); 1 test proves fff hits the real repo |

### Phase E — Tool model

| # | Feature | Status |
|---|---------|--------|
| F19 | Unified tool model — one `ToolDefinition`, `Tool = SingleCommand \| MultiCommands`, `ToolShed { shed, tools: Vec<Tool> }`; delete `MemoryTool`/`DelegationTool` + special slots; providers render the enum (no flattening) | **DONE** — every `Done when` clause verified against the code; the outstanding one (per-provider multi-command rendering coverage) closed by `tests/providers/multi_command_rendering_tests.rs`. Deviation: `MultiCommands` carries the group name. See features/F19-unified-tool-model.md |

### Phase F — GPU execution

| # | Feature | Status |
|---|---------|--------|
| F20 | CUDA end-to-end for candle and llama.cpp | **In progress** — driver skew resolved (610.43.03 both sides); candle and llama.cpp each proven running on CUDA end-to-end. Remaining: the `Done when` list in features/F20-cuda-end-to-end.md (two-GPU `main_gpu`/`tensor_split` runs, offload-count assertion, `gpu-tests` gating). |

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

## Completion status — NOT complete

Phases A, B, C (bar F10) and D are done. The spec stays open on two features:

| # | State | What is left |
|---|---|---|
| F10 | Partial | Candle multi-architecture — Llama + Gemma2 done; Qwen/Mistral/Phi3 need fixtures that do not exist yet |
| F20 | In progress | CUDA runs end-to-end on both backends, but the `Done when` list is unmet — two-GPU `main_gpu`/`tensor_split` runs, the offload-count assertion (a test that passes when the model silently ran on CPU is worse than no test), and `gpu-tests` gating |

Do **not** move this spec to `specifications/completed/` until those three
close. F04 being green is not the same as the spec being done.

## Where the value actually landed

F04 was framed as a coverage number. The number moved (79.00% → 84.99% →
88.03% lines), but the return was **seven real defects**, five of them outside
`foundation_ai`: HF downloads caching vendor error bodies as model files, a
14-site TOCTOU in valtron that silently dropped delivered stream items, an HTTP
drain that waited out the peer's keep-alive idle timeout (over a minute per
request against nginx defaults), a dead-pooled-connection path that fails on the
read rather than the write, `TestHttpServer` silently handing over empty request
bodies, and an external test that could not terminate and wedged whole runs.

Full write-ups, including the wrong diagnoses that came first, are in
[findings.md](findings.md).
