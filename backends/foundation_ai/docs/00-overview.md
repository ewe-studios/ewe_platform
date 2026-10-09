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
| `types/base_types.rs` | `Model`, `ModelProvider`, `ModelId`, `ModelParams`, `ModelInteraction`, `Messages`, `ModelOutput`, `Tool` / `ToolDefinition` / `ToolShed`, `ToolFormatter`, `UsageReport` |
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
       ├─ ToolCallManager     tool registry; builds the ToolShed the model sees
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
| `agentic` *(default)* | The agent layer (today it gates tests/examples; the modules always compile) |
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

These are true of the code today and are worth knowing before you build on it.
Each one is also called out in the doc that covers the area.

1. **Persistent stores can't be used with `AgentSession::build()` or
   `resume()` yet.** Both require `D: Default` and `M: Default`, and only the
   in-memory stores (`MemoryDocumentStore`, `MemoryStorage`) implement
   `Default`. SQL / Turso / D1+R2 / JSON-file stores don't.
2. **The memory hierarchy writes to a different store than context reads
   from.** `build()` gives `MemoryHierarchy` a fresh `M::default()` /
   `D::default()` pair, while `ContextProvider` reads the store you passed to
   `with_memory_store`. With the in-memory stores, working memory written by
   the `memory` tool never reaches the assembled context.
3. **Observation and reflection are not generated automatically.** The loop
   checks the trigger thresholds and emits
   `AgentProgress::ProcessingMemory`, but no code calls a memory model. The
   tiers are filled only through `MemoryHierarchy::persist_observation` /
   `persist_reflection` / `update_working_memory`.
4. **`with_toolshed(...)` with any real tool fails `build()`.** Preflight
   checks that every tool in the shed is registered with the session's
   `ToolCallManager`, which is always empty at build time. Register tools on
   `session.tool_manager()` after `build()` instead (Doc 04).
5. **Tool arguments are not validated against the schema.** The schema goes to
   the model; `ToolCallManager::execute_one` does not check it. Validate
   inside `execute`.
6. **`SessionAccessProvider::can_use_tool` / `can_spend` / `record_usage` are
   never called.** Only `can_access_session`, `can_use_model` and
   `token_budget` are enforced (at `build()`).
7. **Recent messages reach the model newest-first.** `MessageApi::recent`
   returns newest-first and `ContextProvider` appends them in that order; this
   looks like a bug for multi-turn conversations and is not covered by a test.

Doc 15 proposes API changes that remove most of these traps.

## Where to go next

- New users: [`getting-started/`](getting-started/) (providers → harness → tools)
- How a turn runs: [01 — The agentic loop](01-agentic-loop.md)
- Types: [02 — Agent types](02-agent-types.md)
- Backends and routing: [03 — Model providers](03-model-providers.md)
- API direction: [15 — Simplifying the API surface](15-api-simplification-proposal.md)
