---
title: LangGraph.js -- TypeScript Source Architecture
---

# LangGraph.js -- TypeScript Source Architecture

## Purpose

LangGraph.js is the TypeScript/JavaScript port of LangGraph. It implements the same Pregel algorithm, channel system, checkpointing, and state graph API — but with JS-native concurrency primitives (async/await, AbortSignal, Web Streams).

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/langgraphjs/`

## Aha Moments

**Aha: Same algorithm, different concurrency model.** Python uses `ThreadPoolExecutor` and `asyncio.Task` cancellation. JS uses `Promise.race()` with configurable `maxConcurrency` and `AbortSignal`. The BSP superstep logic (`tick()` → `prepare_next_tasks()` → `apply_writes()`) is identical.

**Aha: `AsyncLocalStorage` replaces `contextvars`.** JS uses Node.js `AsyncLocalStorage` (similar to `AsyncLocal` in .NET) for propagating execution context across async boundaries — the same role Python's `contextvars` fills.

**Aha: Web Streams API for streaming.** Instead of Python's `AsyncIterator`, the JS version uses `ReadableStream` and `WritableStream` — the browser-native standard. This means `graph.stream()` returns a `ReadableStream` that works in browsers, Deno, and Node.js.

**Aha: Three SDK families for frontend.** Beyond the core engine, there are SDK bindings for React, Angular, Svelte, and Vue — each with framework-specific hooks for connecting to LangGraph Platform.

## Source Map

```
langgraphjs/libs/
├── langgraph/              # Main package — re-exports from langgraph-core
├── langgraph-core/         # Core engine (Pregel, channels, state graph)
│   └── src/
│       ├── pregel/
│       │   ├── index.ts     # Pregel class (2428 lines)
│       │   ├── loop.ts      # PregelLoop (1322 lines)
│       │   ├── algo.ts      # prepare_next_tasks, apply_writes (1246 lines)
│       │   ├── runner.ts    # PregelRunner (516 lines)
│       │   └── types.ts     # Pregel types
│       ├── channels/
│       │   ├── base.ts      # BaseChannel
│       │   ├── last_value.ts, topic.ts, binop.ts, ...
│       ├── graph/
│       │   ├── state.ts     # StateGraph (1657 lines)
│       │   ├── types.ts     # Graph types
│       │   └── zod/         # Zod schema interop
│       ├── prebuilt/
│       │   ├── index.ts     # createReactAgent, ToolNode
│       │   └── react_agent_executor.ts
│       ├── func/
│       │   └── index.ts     # entrypoint(), task() — functional API
│       └── annotation.ts    # Annotation.Root() declarative schemas
├── checkpoint/             # Base checkpoint + MemorySaver
├── checkpoint-postgres/    # PostgreSQL checkpointer
├── checkpoint-sqlite/      # SQLite checkpointer
├── checkpoint-mongodb/     # MongoDB checkpointer
├── checkpoint-redis/       # Redis checkpointer
├── checkpoint-validation/  # Test utilities
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

## Pregel Runtime (`langgraph-core/src/pregel/index.ts:384`)

```typescript
// pregel/index.ts:384
class Pregel<Nodes, Channels, ContextType, InputType, OutputType>
  extends PartialRunnable<InputType, OutputType>
  implements PregelInterface<...>
```

Extends LangChain's `PartialRunnable` — every graph is a runnable. Key methods:
- `invoke(input, config?)` — run to completion
- `stream(input, config?, streamMode?)` — `ReadableStream` of events
- `getState(config)` — get current checkpoint state
- `getStateHistory(config)` — iterate through checkpoint history
- `updateState(config, values)` — manual state update

Stream modes (line 23):
```typescript
type StreamMode = "values" | "updates" | "debug" | "messages" | "checkpoints" | "tasks" | "custom" | "tools";
```

Durability (line 33):
```typescript
type Durability = "exit" | "async" | "sync";
```

The core execution loop is `_streamIterator()` (line 1974):
1. Initialize `PregelLoop`
2. Create `PregelRunner`
3. Drive tick → run → emit cycle

## PregelLoop (`langgraph-core/src/pregel/loop.ts:216`)

```typescript
// pregel/loop.ts:216
class PregelLoop {
  static async initialize(options) { ... }  // Factory — loads checkpoint, creates channels
  async tick(): Promise<boolean> { ... }     // Single BSP superstep
  async _first(): Promise<void> { ... }      // Handle initial input
  async finishAndHandleError(): Promise<void> { ... }  // Final cleanup
  async putWrites(taskId, writes): Promise<void> { ... }  // Save task results
  async acceptPush(task, writeIdx, call?): Promise<void> { ... }  // Accept dynamic tasks
}
```

Default recursion limit: 25 (line 98).

## P Algo (`langgraph-core/src/pregel/algo.ts`)

Same three-phase BSP as Python:
- `_prepareNextTasks()` (line 454) — PUSH tasks from `Send` + PULL tasks from edges
- `_applyWrites()` (line 245) — apply task outputs to channels atomically
- `_prepareSingleTask()` (line 545) — create executable task from task path

**PUSH vs PULL:** PUSH from `Send` objects (dynamic), PULL from edges (static topology). Same as Python.

## PregelRunner (`langgraph-core/src/pregel/runner.ts:83`)

```typescript
// pregel/runner.ts:83
class PregelRunner {
  async tick(tasks, options): Promise<void> { ... }
  async _executeTasksWithRetry(tasks, options): Promise<void> { ... }
}
```

Key difference from Python:
- Uses `Promise.race()` with a barrier pattern for concurrent execution
- `AbortSignal` for cancellation (line 204)
- `maxConcurrency` config for controlling parallelism
- `call()` function (line 398) pushes sub-tasks dynamically during execution

## Channels (`langgraph-core/src/channels/`)

Same channel types as Python, all implementing `BaseChannel<ValueType, UpdateType, CheckpointType>`:

| File | Class | Purpose |
|------|-------|---------|
| `base.ts:13` | `BaseChannel` | Abstract base — `get()`, `update()`, `checkpoint()`, `consume()`, `finish()` |
| `last_value.ts` | `LastValue` | Stores last single value; errors on concurrent writes |
| `last_value.ts` | `LastValueAfterFinish` | Available only after `finish()` |
| `topic.ts` | `Topic<Value>` | Pub/sub with `unique` and `accumulate` options |
| `any_value.ts` | `AnyValue` | Accepts any value, no conflict resolution |
| `ephemeral_value.ts` | `EphemeralValue` | One-step value, not persisted |
| `untracked_value.ts` | `UntrackedValue` | Not saved to checkpoints |
| `binop.ts` | `BinaryOperatorAggregate` | Reduces with binary operator |
| `dynamic_barrier_value.ts` | `DynamicBarrierValue` | Waits for dynamic set of sources |
| `named_barrier_value.ts` | `NamedBarrierValue` | Waits for fixed set of named sources |

## StateGraph (`langgraph-core/src/graph/state.ts:292`)

```typescript
// graph/state.ts:292
class StateGraph<SD, S, U, N, I, O, C> {
  addNode(name, node, options?): this { ... }
  addEdge(source, target): this { ... }
  addSequence(nodes): this { ... }
  compile(options?): CompiledStateGraph { ... }
}
```

Three schema types supported:
1. **`Annotation.Root()`** — declarative channel definitions (line 254 of `annotation.ts`)
2. **`StateSchema`** — class-based schema
3. **Zod objects** — via `graph/zod/` interop layer

Constructor normalizes all patterns via `_normalizeToStateGraphInit()` (line 458).

## Functional API (`langgraph-core/src/func/index.ts`)

```typescript
// func/index.ts
function task(name, func, options?): TaskFunction<...> { ... }
function entrypoint(name, func, options?): EntrypointFunction<...> { ... }

// entrypoint.final({ value, save }) — separates return value from persisted state
// getPreviousState<StateT>() — accesses state from previous invocation on same thread
```

In JS, these are function calls (not decorators). The underlying mechanism is the same — both create a Pregel graph with a single node.

## Checkpoint System

**Checkpoint interface** (v4 format):
```typescript
interface Checkpoint {
  v: number;           // Version: 4
  id: string;          // UUID
  ts: string;          // ISO 8601 timestamp
  channel_values: Record<string, any>;
  channel_versions: Record<string, number>;
  versions_seen: Record<string, Record<string, number>>;
}
```

**`BaseCheckpointSaver`** (abstract):
- `getTuple(config)` — get checkpoint + metadata
- `list(config, options)` — async generator of checkpoint tuples
- `put(config, checkpoint, metadata)` — save checkpoint
- `putWrites(config, writes, taskId)` — save task writes
- `deleteThread(threadId)` — delete all checkpoints for a thread

Backends: MemorySaver, PostgresSaver, SqliteSaver, MongoDB, Redis.

## Python vs JavaScript Comparison

| Aspect | Python | JavaScript |
|--------|--------|------------|
| Execution model | Sync + Async variants | Async-only (async/await) |
| Concurrency | ThreadPoolExecutor | Promise.race() + maxConcurrency |
| Cancellation | asyncio.Task | AbortSignal |
| Context propagation | contextvars | AsyncLocalStorage |
| Streaming | AsyncIterator | Web ReadableStream |
| Serialization | pickle + JsonPlusSerializer | JsonPlusSerializer only |
| Decorators | @entrypoint, @task | entrypoint(), task() |
| Checkpoint format | v4 | v4 (same) |
| Core algorithm | Pregel supersteps | Identical Pregel supersteps |
| Schema support | TypedDict, Pydantic | Annotation, Zod, classes |

## SDK Ecosystem

**Headless SDK** (`libs/sdk/src/client.ts`): HTTP client for LangGraph Platform
- Thread management, runs, checkpoints, assistants, crons, store
- SSE streaming with `SSEDecoder` and `BytesLineDecoder`
- API key precedence: `LANGGRAPH_API_KEY > LANGSMITH_API_KEY > LANGCHAIN_API_KEY`

**Frontend SDKs**: React hooks, Angular services, Svelte stores, Vue composables — each providing framework-specific bindings for connecting UIs to LangGraph Platform.

**Multi-agent patterns**:
- `langgraph-swarm` — multi-agent swarm using `activeAgent` state key
- `langgraph-supervisor` — supervisor pattern with handoff tools
- `langgraph-cua` — computer use agent

[Back to LangGraph source architecture → ../langchain/05-langgraph-source.md](../langchain/05-langgraph-source.md)
