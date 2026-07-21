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
| F02 | Test-feature gating — `live-model-tests` + `external-service-tests` split | **In progress** — live-model exists; external-service to add |
| F03 | Live-model SmolLM harness — pull-on-demand, cached, offline-skip | Not started |
| F04 | Drive coverage to 100% of critical logic — error modules, generator, providers, harness tail | **In progress** — errors/mod done; generator/llama/responses/harness remain |

### Phase B — Standard agentic tools (VFS-backed)

The `ToolShed` declares `read` / `edit` / `write` / `shell` slots but ships **no
implementations** — the agent literally cannot touch the filesystem or run a
command. These implement them as `ToolImpl`s over the VFS
(`foundation_nativeapis` `VfsFileSystem`) so the FS base is swappable.

| # | Feature | Status |
|---|---------|--------|
| F05 | `read` tool — read a file (range/whole) via VFS | Not started |
| F06 | `write` tool — create/overwrite a file via VFS | Not started |
| F07 | `edit` tool — targeted string replace in a file via VFS | Not started |
| F08 | `bash` tool — run a shell command (native), captured stdout/stderr/exit; target-gated | Not started |
| F09 | Tool defaults wiring — register read/write/edit/bash in the `ToolShed` + `ToolCallManager` defaults, gated by an injected VFS/exec capability | Not started |

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
| F14 | `memory` tool — the `MemoryTool` ToolShed slot has no ToolImpl | 36/F10, F15 | Not started |
| F15 | `delegate` tool — the `DelegationTool` ToolShed slot has no ToolImpl (sub-agent delegation) | 36/F14 | Not started |
| F16 | `search_context` semantic recall wiring — F16 marked recall "deferred to F31/F32 wiring"; verify the tool actually recalls | 36/F16, F32 | Not started |
| F17 | Vector/embedding recall path end-to-end (EmbeddingProvider → semantic search tool) | 36/F31, F24-30 | Not started |

## Working agreements

- No `#[traced_test]` — `#[valtron_test]` handles tracing.
- Tests use foundation capabilities only; elevate reusable glue into the crate
  (e.g. `PreloadedProvider`, `MockTool.with_category`).
- Tools use the VFS so the FS base swaps; native shell is target-gated.
- Every feature: finish 100% (impl + tests + coverage) before the next; mark
  complete in this index and commit.

## Immediate TODO queue (what we're mid-stream on)

1. **F04**: finish 0%/low-coverage files — `errors/llama.rs`, `models/generator.rs`
   (offline request-construction parts + external-service send tests),
   `openai_responses_provider.rs`, `huggingface_*_provider.rs` (live/external),
   `harness/agents.rs` tail, `toolbox/llama_server_harness.rs`.
2. **F02**: add the `external-service-tests` feature; move OpenRouter/cloud/HF-download
   tests under it; keep `live-model-tests` for SmolLM/local-model runs.
3. **F03**: SmolLM pull-on-demand harness for live-model-tests.
4. **F05–F09**: implement read/write/edit/bash tools on the VFS + wire defaults.
5. **F14–F17**: spec-36 carryover — memory tool, delegate tool, semantic recall.
