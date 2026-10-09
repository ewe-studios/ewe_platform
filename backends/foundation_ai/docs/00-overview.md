# foundation_ai — overview

## What it is

The AI layer of ewe-platform. One `Model` trait fronts every inference backend
(cloud HTTP APIs and local llama.cpp / Candle), a `ProviderRouter` picks the
backend for a `ModelId`, and the **agentic** layer (`AgentSession` /
`AgentLoop`) runs multi-step turns on top: context assembly, generation, tool
execution, steering, loop detection, and token accounting.

There is no tokio and no `async fn` on the hot path. Generation and the agent
loop are Valtron `StreamIterator` / `TaskIterator` state machines driven by the
Valtron executor (`#[valtron] fn main` or an equivalent executor setup). Tools
are the one async surface (`ToolImpl::execute` is an `async_trait` method); the
loop drives those futures through Valtron too.

## Module map (`src/`)

| Module | What lives there |
|---|---|
| `types/base_types.rs` | `Model`, `ModelProvider`, `ModelId`, `ModelParams`, `ModelInteraction`, `Messages`, `ModelOutput`, `Tool` / `ToolDefinition` / `ToolDeclarations`, `ToolFormatter`, `UsageReport` |
| `types/routable_provider.rs` | `RoutableProvider` (object-safe), `RoutableProviderBox`, `PreloadedProvider`, `ProviderRouter`, `RoutingRule` |
| `types/agentic.rs` | `SessionId`, `SessionRecord`, memory entry types (`MemoryFact`, `ObservationEntry`, `ReflectionEntry`) |
| `backends/` | The concrete `ModelProvider`s: OpenAI Chat Completions, OpenAI Responses, Anthropic Messages, llama.cpp (+ HuggingFace GGUF download), Candle (+ HuggingFace safetensors download) |
| `models/providers/` | **Generated** model descriptor catalogues (pricing, context window, API kind) for Anthropic, OpenAI, OpenRouter, Bedrock, Vertex, xAI, … — data, not provider implementations. Regenerate with `cargo run --bin ewe_platform gen_model_descriptors` |
| `models/generator.rs` | The catalogue generator (native only) |
| `agentic/` | `AgentSession`, `AgentLoop`, `ContextProvider`, `MessageApi`, `MemoryHierarchy`, `TokenLedger`, `SteeringQueues`, `LoopDetector`, `ErrorPolicy` / `CircuitBreaker`, embeddings, Arrow serialization, the tool runtime (`ToolImpl`, `ToolCallManager`) and the built-in tools (`agentic/tools/`) |
| `harness/` | One-call setup: provider presets (`CloudPresets`, `Glm52`, `Gemma4E2b`, …), `RouterMix` / `RouterPreset`, ready-made `*_session` builders, and `ToolPreset` |
| `costing.rs` | `calculate_cost`, `CostAccumulator`, token estimation helpers |
| `errors/` | `GenerationError`, `ModelProviderErrors`, llama.cpp error wrappers |
| `toolbox/` | llama-server test harness (`toolbox` feature) |

## Architecture

```
AgentSession<D: DocumentStore, M: MemoryStore>      (agentic/session.rs)
  │  run_turn / run_turn_stream / steer / follow_up / abort / end / resume
  │
  └─ per turn: AgentLoop  — a Valtron TaskIterator   (agentic/agent_loop.rs)
       ├─ ProviderRouter      ModelId → RoutableProvider → Box<dyn Model>
       ├─ ContextProvider     system prompt + memory tiers + recent messages
       ├─ MessageApi<D>       append-only session log over a DocumentStore
       ├─ ToolCallManager     tool registry, built from the session's ToolShed; offers `shed` + activated tools
       ├─ MemoryHierarchy     memory tiers + trigger thresholds
       ├─ SteeringQueues      priority / follow-up queues + cancel signal
       ├─ TokenLedger         usage, budget, rolling counter
       ├─ LoopDetector        repetition + vacuous-answer detection
       └─ ErrorPolicy + CircuitBreaker   error → action, fallback models
```

`AgentSession` owns these components; every `run_turn_stream` call builds a
fresh `AgentLoop` that shares them and schedules it on the Valtron executor.

## Feature flags

| Flag | Effect |
|---|---|
| `llamacpp` *(default)* | llama.cpp backend + HuggingFace GGUF provider (native and emscripten) |
| `candle` *(default)* | Candle backend + HuggingFace safetensors provider (native) |
| `cuda` / `cuda_static` / `metal` / `vulkan` | GPU builds of llama.cpp |
| `candle-cuda` (alias `candle-gpu`) | Candle on CUDA (Metal is automatic on Apple) |
| `mtmd` | llama.cpp multimodal |
| `openmp`, `android` | llama.cpp build variants |
| `multi` | Multi-threaded Valtron executor |
| `testing` | `MockModelProvider`, `MockTool`, message builders (`agentic::testing`) |
| `toolbox` | llama-server harness |
| `live-model-tests` | Tests that need a downloaded model |
| `external-service-tests` | Tests that call cloud APIs (skip themselves without credentials) |

## Known limitations

True of the code today, and worth knowing before you build on it:

1. **Observation and reflection are not generated automatically.** The loop
   checks the trigger thresholds and emits
   `AgentProgress::ProcessingMemory`, but no code calls a memory model. Those
   tiers are filled only through `MemoryHierarchy::persist_observation` /
   `persist_reflection` (Doc 11). Working memory works, via the `memory` tool.
2. **Streamed text chunks mean different things per backend.** The HTTP
   backends send the whole text so far on every chunk; llama.cpp and Candle
   send only the new piece. The loop copes (it persists one merged message per
   turn), but a streaming consumer printing chunks must know which backend it
   is talking to (Doc 02 §1).
3. **The crate needs the `llamacpp` feature to compile.** `harness` imports
   llama.cpp types without a feature gate, so `--no-default-features` builds
   fail.

Doc 15 proposes API changes that make the rest of the surface harder to
misuse.

## Where to go next

- New users: [`getting-started/`](getting-started/) (providers → harness → tools)
- How a turn runs: [01 — The agentic loop](01-agentic-loop.md)
- Types: [02 — Agent types](02-agent-types.md)
- Backends and routing: [03 — Model providers](03-model-providers.md)
- API direction: [15 — Simplifying the API surface](15-api-simplification-proposal.md)
