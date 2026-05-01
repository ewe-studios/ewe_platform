---
title: LangGraph.js -- TypeScript Source Architecture
---

# LangGraph.js -- TypeScript Source Architecture

## Purpose

LangGraph.js is the TypeScript/JavaScript port of LangGraph. Same Pregel algorithm, channel system, checkpointing, and state graph API — but with JS-native concurrency (async/await, AbortSignal, Web Streams).

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/langgraphjs/`

## Aha Moments

**Aha: Same algorithm, different concurrency.** Python uses `ThreadPoolExecutor` and `asyncio.Task` cancellation. JS uses `Promise.race()` with configurable `maxConcurrency` and `AbortSignal`. The BSP superstep logic is identical.

**Aha: `AsyncLocalStorage` replaces `contextvars`.** Node.js `AsyncLocalStorage` propagates execution context across async boundaries — the same role Python's `contextvars` fills.

**Aha: Web Streams API for streaming.** Instead of Python's `AsyncIterator`, JS uses `ReadableStream`/`WritableStream` — the browser-native standard.

**Aha: Three frontend SDK families.** React hooks, Angular services, Svelte stores, Vue composables — each with framework-specific bindings for LangGraph Platform.

## Monorepo Structure

```
langgraphjs/libs/
├── langgraph/              # Main — re-exports from langgraph-core
├── langgraph-core/         # Core engine (Pregel, channels, state graph)
├── checkpoint/             # Base checkpoint + MemorySaver
├── checkpoint-postgres/    # PostgreSQL checkpointer
├── checkpoint-sqlite/      # SQLite checkpointer
├── checkpoint-mongodb/     # MongoDB checkpointer
├── checkpoint-redis/       # Redis checkpointer
├── sdk/                    # Headless JS client for LangGraph Platform
├── sdk-react/              # React hooks
├── sdk-angular/            # Angular services
├── sdk-svelte/             # Svelte stores
├── sdk-vue/                # Vue composables
├── langgraph-swarm/        # Multi-agent swarm pattern
├── langgraph-supervisor/   # Supervisor multi-agent pattern
├── langgraph-cua/          # Computer Use Agent
├── langgraph-api/          # Server-side API (LangGraph Platform)
├── langgraph-cli/          # CLI tooling
├── langgraph-ui/           # UI components
└── create-langgraph/       # Project scaffolding CLI
```

## Core Engine (`langgraph-core/src/`)

### Pregel (`pregel/index.ts:384`, 2428 lines)

```typescript
class Pregel extends PartialRunnable implements PregelInterface
```

Extends LangChain's `PartialRunnable` — every graph is a runnable. Key methods: `invoke()`, `stream()` (returns `ReadableStream`), `getState()`, `getStateHistory()`, `updateState()`.

Stream modes: `"values" | "updates" | "debug" | "messages" | "checkpoints" | "tasks" | "custom" | "tools"`

Durability: `"exit" | "async" | "sync"`

### PregelLoop (`pregel/loop.ts:216`, 1322 lines)

```typescript
class PregelLoop {
  static async initialize(options): Promise<PregelLoop>
  async tick(): Promise<boolean>       // Single BSP superstep
  async _first(): Promise<void>        // Handle initial input
  async putWrites(taskId, writes)      // Save task results
  async acceptPush(task, writeIdx)     // Accept dynamic tasks
}
```

Default recursion limit: 25.

### PregelRunner (`pregel/runner.ts:83`, 516 lines)

- `Promise.race()` with barrier pattern for concurrent execution
- `AbortSignal` for cancellation
- `maxConcurrency` config for controlling parallelism
- `call()` pushes sub-tasks dynamically during execution

### Channels (`channels/`)

Same types as Python, implementing `BaseChannel<ValueType, UpdateType, CheckpointType>`:

| Class | Purpose |
|-------|---------|
| `LastValue` | Stores last single value |
| `LastValueAfterFinish` | Available only after `finish()` |
| `Topic<Value>` | Pub/sub with `unique` and `accumulate` |
| `AnyValue` | Accepts any value |
| `EphemeralValue` | One-step, not persisted |
| `UntrackedValue` | Not saved to checkpoints |
| `BinaryOperatorAggregate` | Reduces with binary operator |
| `DynamicBarrierValue` | Waits for dynamic set of sources |
| `NamedBarrierValue` | Waits for fixed set of named sources |

### StateGraph (`graph/state.ts:292`, 1657 lines)

```typescript
class StateGraph {
  addNode(name, node, options?): this
  addEdge(source, target): this
  addSequence(nodes): this
  compile(): CompiledStateGraph
}
```

Three schema types: `Annotation.Root()` (declarative), `StateSchema` (class-based), Zod objects.

### Functional API (`func/index.ts`, 463 lines)

```typescript
function task(name, func, options?): TaskFunction
function entrypoint(name, func, options?): EntrypointFunction
// entrypoint.final({ value, save }) — separates return from persisted state
// getPreviousState<StateT>() — accesses state from previous invocation
```

In JS, function calls (not decorators). Same underlying mechanism.

## Python vs JavaScript

| Aspect | Python | JavaScript |
|--------|--------|------------|
| Execution | Sync + Async | Async-only |
| Concurrency | ThreadPoolExecutor | Promise.race() + maxConcurrency |
| Cancellation | asyncio.Task | AbortSignal |
| Context | contextvars | AsyncLocalStorage |
| Streaming | AsyncIterator | Web ReadableStream |
| Serialization | pickle + JsonPlus | JsonPlus only |
| Checkpoint format | v4 | v4 (same) |
| Core algorithm | Pregel supersteps | Identical |

[Back to LangGraph source → ../langchain/05-langgraph-source.md](../langchain/05-langgraph-source.md)
