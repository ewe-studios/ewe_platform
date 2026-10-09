# foundation_ai — AI model providers and agentic loop

## What it is
The AI layer: model providers (OpenAI, Anthropic, Llama.cpp, Candle), the agentic
loop, tool execution, memory hierarchy, and context assembly.

## Key modules
- **`backends/`** — Model provider implementations: OpenAI, Anthropic Messages,
  Llama.cpp (local), Candle (local ML framework), OpenAI Responses API.
- **`harness/`** — One-call model setup: pre-configured provider presets,
  `RouterMix` for mixing heterogeneous providers, and ready-to-customize agent
  builders for common combinations (see Doc 12).
- **`agentic/`** — The agentic layer:
  - `loop_detection.rs` — Repetition detection with escalation ladder
  - `memory.rs` / `memory_coordinator.rs` — Working memory, observation, reflection
  - `context.rs` — Prompt assembly with memory-trigger firing
  - `token_ledger.rs` — Token accounting and budget enforcement
  - `access.rs` — Session access control and budget tracking
  - `tool_impl.rs` — Tool registry and execution DAG
  - `tools/search.rs` — Context search and file search tools
- **`costing.rs`** — Model cost calculation and usage tracking
- **`types/`** — Core types: Messages, ModelOutput, VectorEntry, etc.

## Architecture
```
AgentSession
  └── AgenticLoop (orchestrates generation + memory + tools)
        ├── ModelProviderRouter (selects provider per model)
        ├── ToolCallManager (registers + executes tools)
        ├── MemoryHierarchy (working/observation/reflection)
        ├── ContextProvider (assembles prompts from memory)
        └── TokenLedger (tracks + enforces budgets)
```

## Feature flags
- `llamacpp` — Local inference via llama.cpp
- `agentic` — Agentic loop and tool execution
- `candle` — Local ML via Candle framework
- `testing` — MockModelProvider and MockTool for tests
- `metal` / `vulkan` / `cuda` — GPU acceleration backends
