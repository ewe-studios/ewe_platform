# LEARNINGS — LlamaCpp Embeddings

_Seeded at spec creation from the review that found the bug. Updated after each
milestone/task. See `.agents/skills/specifications-management/skill.md`._

## Key finding (creation, 2026-10-18)

The local llama.cpp embedding branch returns `EmbeddingsError::NotEnabled`
unconditionally, so **no local embedding request can ever succeed**. Two
independent context params are both missing, and *either one alone* is not
enough:

1. `embeddings = true` — gates `LlamaModelContext.embeddings_enabled`, which
   `embeddings_seq_ith` checks first (`context.rs:148-151`). Without it the call
   short-circuits.
2. `pooling_type` — even with embeddings enabled, `llama_get_embeddings_seq`
   returns null (→ `NonePoolType`) unless a pooling type (Mean/CLS/Last) is set,
   because the default is unspecified/none.

`LlamaBackendConfig::to_context_params` sets neither, because it is written for
generation. The stale doc comment at `llamacpp.rs:192-198` — "generation and
embeddings share one context" — misled the diagnosis: the embedding branch
actually builds a **separate** context (`llamacpp.rs:524-527`), which is what
makes the fix safe (embedding params can be applied without affecting
generation).

## Why the OpenAI path works

`openai_provider.rs` has its own `is_embedding_request` + embedding handling, so
embeddings were "working" end-to-end only through remote providers. The local
path was silently broken, which is why no test caught it — there was no GGUF
integration test exercising `Model::generate` with an embedding-shaped
interaction.

## "Supports embeddings" is model-shaped, not provider-shaped

Embedding is a property of the **(engine, model) pair**, not the provider:
OpenAI has a real embeddings API; Anthropic has none; llama.cpp can *compute* an
embedding for any loaded model, but only a real embedding model yields
meaningful vectors. The codebase does not model this — `Model::generate` infers
"is this an embedding request?" from the request shape (`is_embedding_request`),
not from a capability the model advertises. Making that capability explicit is
deliberately out of scope for this spec (recorded in requirements.md "Out of
scope").

## Raw text, not the chat template

Embedding input should be the **raw user text**. The original branch fed the
chat-templated prompt to the encoder (chat-control tokens add noise). R3 makes
the embedding path extract raw text before `apply_chat_template` runs. Watch the
BOS token: `generate_embeddings` currently tokenizes with `AddBos::Always`,
which is wrong for BERT-style embedding models — expect to switch the embedding
path to `AddBos::Never`.

## Gotcha for the fix — interaction shape

The embedding interaction shape is non-obvious: `make_embedding_interaction`
(`agentic/embedding.rs:283`) places an `Assistant` message carrying an empty
`ModelOutput::Embedding` as a *marker*, followed by the `User` text. The marker
is what `is_embedding_request` keys on, and `apply_chat_template` already skips
it (`llamacpp.rs:1211,1223`). The integration test must mirror this exact shape
or the embedding branch won't trigger.

## Test model — `nomic-embed-text-v1.5`

768 dims, Mean pooling, Apache-2.0, ~137 MB Q8. The standard llama.cpp/Ollama
embedding model (BERT-style) with a known-good GGUF. Commit it under
`artefacts/models/` (convention: `nomic-ai--nomic-embed-text-v1.5-GGUF/`), and
the integration test skips if the file is absent, matching `mtp_generate`.

## Verification command

```
cargo test -p foundation_ai --features integration_tests --test foundation_ai_tests -- --nocapture harness::integrations::llamacpp_embeddings
```

The test asserts non-empty values with `dimensions == 768` — proving *plumbing*
(no `NotEnabled`, correct dims), not retrieval *quality*.

## Implementation (2026-10-20)

R1–R4 implemented in `backends/foundation_ai`:

- **R1** — `LlamaBackendConfig.pooling_type: LlamaPoolingType` (default `Mean`),
  builder `with_pooling_type`; import widened to `{LlamaModelContextParams, LlamaPoolingType}`.
- **R2** — `to_embedding_context_params()` chains `to_context_params().with_embeddings(true)
  .with_pooling_type(self.pooling_type)`; the `generate()` embedding branch now builds its context
  from it (was the generation `ctx_params`). Stale "share one context" comment corrected.
- **R3** — the embedding branch is detected **before** `apply_chat_template`; new
  `embedding_text()` returns the raw `User`/`ToolResult` text (skips the `Assistant` marker +
  images), and `generate_embeddings` tokenizes with `AddBos::Never` (was `Always`).
- **R4** — `tests/harness/integrations/llamacpp_embeddings.rs`: a no-model `#[test]` asserting
  `embeddings()`/`pooling_type()`, plus a `#[valtron_test]` e2e that skips when the GGUF is absent.

### Two corrections to the original spec, found at implementation

1. **Do not commit the model.** `*.gguf` and `artefacts/models/*` are gitignored — the repo never
   commits real weights (only tiny random-weight fixtures under `artefacts/test-models/`, which are
   for the *candle* suite). The nomic GGUF is fetched at run time into `artefacts/models/`
   (`nomic-ai--nomic-embed-text-v1.5-GGUF/nomic-embed-text-v1.5.Q8_0.gguf`); the e2e test skips when
   absent, matching `mtp_generate`.
2. **Full test build is blocked on this machine** by a pre-existing `gemm-f16` failure
   (`instruction requires: fullfp16` — the candle/Metal `gemm-f16` v0.19 inline asm needs
   `+fullfp16` on aarch64). `cargo check -p foundation_ai --lib` and `--test foundation_ai_tests
   --features live-model-tests` both pass; `cargo test` cannot link until the gemm-f16 issue is
   resolved. Unrelated to this spec.
