# Progress - 01 llama.cpp Integration

_Last updated: 2026-04-12_

**Status:** 🔄 In Progress — 18 / 27 tasks (67%)

Integrate llama.cpp as a first-class inference backend in `foundation_ai`
via the `infrastructure_llama_cpp` safe wrapper.

## Done

- [x] **Task Group 1: Type Extensions** — `ModelOutput::Embedding`,
  `ChatMessage`, `LlamaConfig`, `SplitMode`, `KVCacheType`, `llama` on
  `ModelConfig`
- [x] **Task Group 2: Error Extensions** — `GenerationError` variants
  (LlamaCpp, Tokenization, Decode, Encode, ChatTemplate) and `ModelErrors`
  load variants
- [x] **Task Group 3: Sampler Chain Builder** — `build_sampler_chain()` in
  `llamacpp_helpers.rs`, tested
- [x] **Task Group 4: Provider Config** — `LlamaBackendConfig` builder
  with sensible defaults, `LlamaBackends::create()`
- [x] **Task Group 5: Core Model Impl** — `LlamaModels` struct with
  interior mutability, `get_model()`, `spec()`, `costing()`
- [x] **Task Group 6 (partial)** — `Model::generate()`, EOS/stop detection,
  chat template application from `ModelInteraction`, embedding generation
- Recent commit `86c85840 ADD: ai llamacpp rewire` — 264-line rewrite of
  `backends/foundation_ai/src/backends/llamacpp.rs` plus infrastructure
  context param cleanup

## Remaining

- [ ] `Model::stream()` returning `LlamaCppStream`
- [ ] `LlamaCppStream` struct implementing `StreamIterator`
- [ ] Model cache behaviour tests (once cache is added — currently deferred)
- [ ] Enable `#[ignore]`d integration tests (model load, generation, chat,
  embeddings) with a test GGUF fixture
- [ ] Full verification gate:
  `cargo check/clippy/test/fmt --package foundation_ai`
- [ ] Acceptance-criteria checklist in `feature.md` (all items)

## Next Action

Implement `LlamaCppStream` as a Valtron `StreamIterator` and wire
`Model::stream()` to produce it. This is the last functional gap before
the feature can go through verification and be marked complete.

See [`feature.md`](./feature.md) for the full task groups and the
llama.cpp architecture reference.
