---
feature: "foundation_ai: async generate and embed, agent tool busy loop, capped error bodies, flush races, names, test deps"
status: "pending"
priority: "high"
depends_on: ["03-client-body-readiness"]
branch: "fix/io-16-ai"
crates: ["foundation_ai"]
backlog: "docs/bug-backlog.md §10 (9 entries)"
---

# Feature 16: `foundation_ai`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** WebAssembly (wasm); Hypertext Transfer Protocol (HTTP);
input/output (I/O); pull request (PR); AeroSpace and Defence Industries Association of Europe
(ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4. One
commit per entry. Valtron only; no tokio.

## T0. Check that every entry is still open (table in the PR body)

## E1. Non-streaming `generate` does not work on wasm

- **Problem:** providers' sync `generate` calls `HttpClient::send`, which returns an error on wasm;
  the retry back-off `backend_utils::sleep` is a no-op on wasm.
- **Fix:** an async `generate` on the model trait (async-first, `CLAUDE.md` rule 6) over
  `send_async` with async retry back-off (valtron timer). The sync form, if kept, is a thin
  `.block()` at real sync boundaries only. Update callers.
- **Tests:** `generate_async_returns_completion` (native, mock provider);
  `generate_async_retries_with_async_backoff` (a ticker task keeps ticking during back-off);
  a wasm test (`wasm-bindgen-test`, Node) that `generate` succeeds against a mocked `fetch`.

## E2. The `agent` tool busy-loops on `Stream::Wait`

- **Where:** `src/agentic/tools/agent.rs` (~line 492) drains a sub-agent stream and `continue`s
  on `Init | Ignore | Wait | Delayed`.
- **Fix:** drive the sub-agent as its own task and `valtron::park_on` it, or return `Working` on
  `Wait` as it does on `Pending`. Choose the one that keeps the tool's contract; say why.
- **Test:** `agent_tool_does_not_spin_while_sub_agent_waits` (sub-agent waits 500 ms on I/O;
  count the tool's polls < 10; CPU < 50 ms).

## E3. HTTP providers read whole error bodies

- **Where:** non-2xx paths in `openai_provider`, `openai_responses_provider`,
  `anthropic_messages_provider`, `backends::model_info`.
- **Fix:** read at most `max_error_body` bytes (provider config field, default 64 KiB, as Go
  `net/http`'s error-drain limit), status first.
- **Test:** `provider_error_body_is_capped` per provider (a 1 MiB error body; the message is the
  first part; the read stops at the cap).

## E4. `ResponsesProvider::fetch_model_info` has no success test

- **Fix:** add the test that uses `GPT_JSON` in `tests/models/provider_model_info_tests.rs`
  (a `GET` with the bearer token that parses it), as the Anthropic one does.

## E5. Embeddings are generated synchronously inside async code

- **Where:** `EmbeddingProvider::embed` is sync; `CachedEmbeddingProvider::embed_single`
  (`src/agentic/embedding.rs`) calls sync `generate`; called from `ContextProvider::search` and
  the `shed` tool.
- **Fix:** async `embed` / `embed_batch` over E1's async generate; callers `.await`.
- **Test:** `embedding_search_does_not_block_worker` (an HTTP embedder that answers after 300 ms;
  a ticker on a 2-worker pool keeps ticking).

## E6. A flush that finds another flush running returns `Ok(0)` at once

- **Where:** `MessageApi::flush` (`src/agentic/message_api.rs`) guards with an `AtomicBool`.
- **Fix:** the second caller awaits the running flush (an async lock or a completion signal).
- **Test:** `second_flush_waits_for_running_flush` (assert the agent loop does not re-enter
  `Flushing` repeatedly: count entries ≤ 2).

## E7. A cancelled flush can write a record twice

- **Fix:** idempotent writes (upsert on the document id for `append_with_id`; stable ids for
  memory records).
- **Test:** `cancelled_flush_does_not_duplicate_records` (drop the flush future after the store
  accepted the write; flush again; assert one record).

## E8. `MemoryCoordinator` keeps `_async` names

- **Fix:** rename `record_async` → `record`, `hydrate_async` → `hydrate`; update callers.

## E9. Test targets need llama.cpp even without `llamacpp`

- **Where:** `Cargo.toml`: `infrastructure_llama_cpp` is an unconditional native dev-dependency
  (for `errors/llama.rs` tests).
- **Fix:** drop the dev-dependency; reach the llama.cpp error types through a `llamacpp`-gated
  re-export; gate those tests on the feature.
- **Verify:** `cargo test -p foundation_ai --no-default-features --features testing --no-run`
  builds without the `tools/llama.cpp` submodule (test with the submodule directory moved away,
  then restore it).

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_ai --profile dev        # includes wasm
for c in answerme-agent foundation_vectors; do python3 scripts/ci/ci.py run $c --profile dev || exit 1; done
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] T0 table; every open entry fixed; backlog entries deleted.
- [ ] New tests fail on the base code and pass now; 20-run flake check passes.
- [ ] CI green (native and wasm).
