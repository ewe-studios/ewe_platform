# 60: agentic-reliability

Make the agentic loop provably correct: fix the generation-quality defect, put
llama.cpp's output under our log filter, clear the known warts, and prove the
whole session/loop surface with a fast in-process provider (candle).

## North star

**The agentic session and loop are fully validated by tests that run in-process,
without network or multi-GB model downloads, at ~90% coverage of critical logic —
and the local model path produces coherent output.**

"Critical logic" means every state transition, guard, and error path in
`AgentLoop` + `AgentSession`. Trivial accessors and `Debug` impls may be
excluded by review; branch behaviour, error propagation, and flow control may
not.

## Background — why this spec exists

Spec 60 follows a session that found the agent returning `(no response)` to
every prompt. The chain of defects is written up in
`backends/foundation_ai/docs/fixes/006_root_cause_stream_never_creates_context.md`.
All of the following are **already fixed** and are context, not scope:

- worker-thread tracing was dead (`#[valtron]` initialised the pool before the
  subscriber, so workers pinned the no-op dispatcher)
- `LlamaCppStream` never created its inference context (lazy-init gated on the
  wrong field), so every stream died on its second poll
- `LlamaModelContext` was `Clone` **and** `Drop`-freed a raw pointer — a
  use-after-free that segfaulted once the stream actually ran
- sampled tokens were never decoded back, so the KV cache never advanced
- `stream()` ignored `interaction.messages` and applied no chat template
- ~10 failure paths reported success (`Finished` / bare `None`)
- the model was re-loaded from disk on every turn (no cache anywhere)

The lesson driving this spec: **`run_turn` — the session's primary API — had no
test at any level**, and nothing anywhere exercised `Model::stream`. Every
provider suite called `generate()`, a different code path. A whole class of
defects lived in the gap.

## Scope

| # | Workstream | Outcome |
|---|-----------|---------|
| A | Generation quality | Understand and fix why the local model answers with `"."`; assertions that would have caught it |
| B | llama.cpp logging | Silent by default; enabled by an explicit tracing filter directive |
| C | Known warts | Cold-start cache race; any silent-failure paths remaining outside the stream |
| D | Candle test suite | Deep in-process coverage of `AgentSession` + `AgentLoop` to ~90% of critical logic |
| E | llama.cpp version bump | Land **after** A–D so any regression is attributable |

## Progress

| Workstream | Status | Notes |
|-----------|--------|-------|
| A: generation quality | Not started | Root cause unknown — see requirements |
| B: llama.cpp logging | Not started | Lever identified: `send_logs_to_tracing` |
| C: known warts | Not started | Cache race is known and measured |
| D: candle test suite | Not started | The bulk of the work |
| E: llama.cpp bump | Not started | Explicitly last |

## Decisions

| # | Decision | Status |
|---|----------|--------|
| 00 | Test provider is **candle**, in-process, no network at test time | Resolved |
| 01 | Mocks (`agentic::testing::MockModelProvider`) cover branch/flow; a real provider covers the seam mocks cannot see | Resolved |
| 02 | llama.cpp logs route through `tracing`, silent unless a filter directive enables them | Resolved |
| 03 | Coverage target ~90% of critical logic; exclusions require review, not blanket ignores | Resolved |
| 04 | llama.cpp version bump lands last | Resolved |
| 05 | Which candle model is the test model (size/licence/determinism) | **Open** |
| 06 | Coverage measurement tool (`cargo-llvm-cov` vs `tarpaulin`) and whether it gates CI | **Open** |
| 07 | Whether generation-quality assertions can be deterministic (seeded sampler) or must be tolerant | **Open** |

## Non-goals

- Rewriting the agent loop's state machine. Bugs get fixed; the design stands.
- Making `generate()` and `stream()` share one implementation. They should agree
  on *behaviour* and be tested as such; unifying them is a separate question.
- New agentic capability. This spec buys correctness and confidence, not features.

## Related

- `backends/foundation_ai/docs/fixes/006_root_cause_stream_never_creates_context.md` — the defect chain that motivated this
- `backends/foundation_ai/docs/fixes/005_root_cause_ffi_pointers_not_send.md` — why the context must be born on the polling thread
- `backends/foundation_ai/tests/agentic/integrations/session_turn.rs` — the six tests written during the investigation; the seed of workstream D
- `specifications/51-llama-mtp-speculative` — MTP/speculative decoding, shares the stream path
