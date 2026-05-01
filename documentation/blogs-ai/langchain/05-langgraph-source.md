---
title: LangGraph -- Source Code Architecture Deep Dive
---

# LangGraph -- Source Code Architecture Deep Dive

## Purpose

This document reverse-engineers LangGraph from its actual source code (`langgraph==0.6+` at `libs/langgraph/`) to understand how the Pregel runtime, channels, checkpointing, and state management actually work. All claims are grounded in specific source files and line numbers.

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/langgraph/`

## Aha Moments

**Aha: Pregel is a class, not just a pattern.** `Pregel` in `langgraph/pregel/main.py:343` is a concrete class inheriting from `PregelProtocol` and `Generic[StateT, ContextT, InputT, OutputT]`. `StateGraph` compiles down to `CompiledStateGraph` which wraps a `Pregel` instance.

**Aha: The BSP loop is `PregelLoop.tick()` — one iteration per superstep.** `_loop.py:506` implements `tick()` which executes exactly one BSP iteration: plan tasks → execute → apply writes → checkpoint. The caller (sync/async variant) loops until `tick()` returns `False`.

**Aha: Channels are the entire state machine.** Not dicts, not JSON blobs — first-class objects with `update()`, `get()`, `checkpoint()` methods. `LastValue` stores one value, `Topic` accumulates values, `BinaryOperatorAggregate` applies a reducer. Every state key maps to a channel instance.

**Aha: `versions_seen` is how nodes get scheduled.** The checkpoint tracks `{node_id: {channel_name: version}}`. When a channel's version exceeds what a node has seen, that node is selected for execution. This is the Pregel "plan" phase — no hardcoded edges needed.

**Aha: `Command` is the structured control flow.** `Command(goto=...)` replaces hard-coded edges. It can `goto` a node, `Send` to a specific node with data, or `interrupt()` for human-in-the-loop. This makes graphs dynamic — the agent decides the next node at runtime.

**Aha: Durability is configurable per-run.** `Durability = Literal["sync", "async", "exit"]` (`types.py:85`). `"sync"` blocks until checkpoint is saved, `"async"` checkpoints in background while next step runs, `"exit"` only saves when the graph exits. Trade latency vs. durability per invocation.

## Source Map

```
langgraph/
├── pregel/
│   ├── main.py          # Pregel class — the runtime
│   ├── _loop.py         # PregelLoop.tick() — BSP superstep
│   ├── _algo.py         # prepare_next_tasks, apply_writes
│   ├── _read.py         # PregelNode, ChannelRead
│   ├── _write.py        # ChannelWrite, ChannelWriteEntry
│   ├── _checkpoint.py   # create_checkpoint, copy_checkpoint
│   ├── _executor.py     # BackgroundExecutor, Submit
│   ├── _runner.py       # PregelRunner — task execution
│   ├── _io.py           # map_input, map_output_values
│   ├── _validate.py     # validate_graph
│   ├── _draw.py         # Mermaid graph generation
│   └── protocol.py      # PregelProtocol, StreamProtocol
├── graph/
│   ├── state.py         # StateGraph, CompiledStateGraph
│   ├── message.py       # add_messages reducer
│   ├── _branch.py       # BranchSpec
│   └── _node.py         # StateNode, StateNodeSpec
├── channels/
│   ├── base.py          # BaseChannel[Value, Update, Checkpoint]
│   ├── last_value.py    # LastValue, LastValueAfterFinish
│   ├── topic.py         # Topic (pub/sub accumulator)
│   ├── ephemeral_value.py # EphemeralValue (single-use)
│   ├── binop.py         # BinaryOperatorAggregate
│   ├── any_value.py     # AnyValue (accepts any update)
│   ├── untracked_value.py # UntrackedValue (not in checkpoints)
│   └── named_barrier_value.py # NamedBarrierValue, NamedBarrierValueAfterFinish
├── func/
│   └── __init__.py      # @task, @entrypoint — functional API
├── checkpoint/
│   ├── base/__init__.py # BaseCheckpointSaver, Checkpoint, CheckpointMetadata
│   ├── memory/__init__.py # MemorySaver (in-memory)
│   └── serde/
│       ├── jsonplus.py  # JsonPlusSerializer (ormsgpack + fallbacks)
│       └── encrypted.py # EncryptedSerializer
├── store/
│   └── base/__init__.py # BaseStore — long-term memory
├── types.py             # Command, Send, Interrupt, RetryPolicy, etc.
├── runtime.py           # Runtime, ExecutionInfo, ServerInfo
├── config.py            # get_config, configuration helpers
└── callbacks.py         # GraphInterruptEvent, GraphResumeEvent
```

## The Pregel Runtime

### Pregel Class (`pregel/main.py:343-483`)

```python
# pregel/main.py:343
class Pregel(PregelProtocol[StateT, ContextT, InputT, OutputT], Generic[StateT, ContextT, InputT, OutputT]):
    """Pregel manages the runtime behavior for LangGraph applications."""
```

The docstring explains the algorithm directly:

> Pregel combines **actors** and **channels** into a single application.
> **Actors** read data from channels and write data to channels.
> Each step follows the **Pregel Algorithm** / **Bulk Synchronous Parallel** model.

Three phases per step (from the docstring):
1. **Plan**: Determine which actors to execute — nodes subscribed to channels updated in the previous step
2. **Execution**: Execute all selected actors in parallel until all complete, or one fails, or timeout
3. **Update**: Apply channel writes from actors — synchronization barrier

Key attributes:
```python
nodes: Mapping[str, PregelNode]           # Actors (name → node)
channels: Mapping[str, BaseChannel]       # Communication channels
input_channels: str | Sequence[str]       # Where input is written
output_channels: str | Sequence[str]      # Where output is read
checkpointer: BaseCheckpointSaver | None  # Persistence
store: BaseStore | None                   # Long-term memory
cache: BaseCache | None                   # Task result cache
```

### NodeBuilder (`pregel/main.py:179-341`)

The builder pattern for constructing `PregelNode`s:

```python
# pregel/main.py:179
class NodeBuilder:
    __slots__ = ("_channels", "_triggers", "_tags", "_metadata",
                 "_writes", "_bound", "_retry_policy", "_cache_policy")

    def subscribe_only(self, channel: str) -> Self: ...     # Subscribe to one channel
    def subscribe_to(self, *channels: str, read: bool = True) -> Self: ...  # Subscribe to many
    def read_from(self, *channels: str) -> Self: ...        # Read without subscribing
    def do(self, node: RunnableLike) -> Self: ...           # Set the executable
    def write_to(self, *channels: str | ChannelWriteEntry, **kwargs) -> Self: ...  # Channel writes
    def meta(self, *tags: str, **metadata: Any) -> Self: ... # Tags and metadata
    def add_retry_policies(self, *policies: RetryPolicy) -> Self: ...
    def add_cache_policy(self, policy: CachePolicy) -> Self: ...
    def build(self) -> PregelNode: ...                       # Build the node
```

This is the low-level API. Most users go through `StateGraph` instead.

### PregelLoop (`pregel/_loop.py:148-210`)

The loop state machine:

```python
# pregel/_loop.py:148
class PregelLoop:
    config: RunnableConfig
    step: int          # Current superstep number
    stop: int          # Max steps (recursion limit)
    status: Literal["input", "pending", "done", "interrupt_before", "interrupt_after", "out_of_steps"]
    tasks: dict[str, PregelExecutableTask]
    checkpoint: Checkpoint
    checkpoint_pending_writes: list[PendingWrite]
    updated_channels: set[str] | None  # Channels modified this step
```

The core iteration (`_loop.py:506-619`):

```python
# pregel/_loop.py:506
def tick(self) -> bool:
    """Execute a single iteration of the Pregel loop. Returns True if more iterations needed."""

    # 1. Check step limit
    if self.step > self.stop:
        self.status = "out_of_steps"
        return False

    # 2. PLAN: Determine which tasks to execute
    self.tasks = prepare_next_tasks(
        self.checkpoint, self.checkpoint_pending_writes,
        self.nodes, self.channels, ...,
        trigger_to_nodes=self.trigger_to_nodes,
        updated_channels=self.updated_channels,
    )

    # 3. If no tasks, we're done
    if not self.tasks:
        self.status = "done"
        return False

    # 4. Check interrupt_before
    if self.interrupt_before and should_interrupt(...):
        self.status = "interrupt_before"
        raise GraphInterrupt()

    # 5. EXECUTE: Tasks run in parallel (handled by caller — SyncPregelLoop/AsyncPregelLoop)

    # 6. After execution (after_tick):
    #    Apply writes to channels
    self.updated_channels = apply_writes(
        self.checkpoint, self.channels, self.tasks.values(), ...
    )
    #    Checkpoint
    self._put_checkpoint({"source": "loop"})
    #    Check interrupt_after
    if self.interrupt_after and should_interrupt(...):
        self.status = "interrupt_after"
        raise GraphInterrupt()
```

### Task Planning (`pregel/_algo.py`)

`prepare_next_tasks` determines which nodes to run each superstep:

```python
# pregel/_algo.py (conceptual)
def prepare_next_tasks(checkpoint, pending_writes, nodes, channels, ...,
                       trigger_to_nodes, updated_channels):
    """
    For each channel that was updated (updated_channels):
        For each node subscribed to that channel (trigger_to_nodes):
            If the node hasn't seen this channel version yet:
                Schedule the node for execution
    """
    # The key logic: versions_seen tracks what each node has processed
    # A node runs when: channel_version > versions_seen[node][channel]
```

The `versions_seen` map in `Checkpoint` is what makes this work:

```python
# checkpoint/base/__init__.py:88
Checkpoint:
    versions_seen: dict[str, ChannelVersions]
    """Map from node ID to map from channel name to version seen.
    This keeps track of the versions of the channels that each node has seen.
    Used to determine which nodes to execute next."""
```

## Channels — The State Machine

### BaseChannel (`channels/base.py:19-121`)

```python
# channels/base.py:19
class BaseChannel(Generic[Value, Update, Checkpoint], ABC):
    """Base class for all channels."""

    @property
    @abstractmethod
    def ValueType(self) -> Any:
        """The type of the value stored in the channel."""

    @property
    @abstractmethod
    def UpdateType(self) -> Any:
        """The type of the update received by the channel."""

    def copy(self) -> Self: ...
    def checkpoint(self) -> Checkpoint: ...
    def from_checkpoint(self, checkpoint) -> Self: ...

    @abstractmethod
    def get(self) -> Value: ...
    def is_available(self) -> bool: ...

    @abstractmethod
    def update(self, values: Sequence[Update]) -> bool: ...
    def consume(self) -> bool: ...     # Notify that a subscribed task ran
    def finish(self) -> bool: ...      # Notify that the Pregel run is finishing
```

Three type parameters:
- `Value`: The type you get when you `get()` the channel
- `Update`: The type of values passed to `update()`
- `Checkpoint`: The serializable state saved at checkpoints

### LastValue (`channels/last_value.py:20-78`)

The default channel — stores the last value sent:

```python
# channels/last_value.py:20
class LastValue(Generic[Value], BaseChannel[Value, Value, Value]):
    """Stores the last value received, can receive at most one value per step."""

    def update(self, values: Sequence[Value]) -> bool:
        if len(values) == 0:
            return False
        if len(values) != 1:
            raise InvalidUpdateError(
                "Can receive only one value per step. Use an Annotated key to handle multiple values."
            )
        self.value = values[-1]
        return True

    def get(self) -> Value:
        if self.value is MISSING:
            raise EmptyChannelError()
        return self.value
```

Key insight: if multiple nodes write to a `LastValue` channel in the same step, it raises `InvalidUpdateError`. You need `Annotated[Type, reducer]` (like `add_messages`) to handle concurrent writes.

### Topic (`channels/topic.py:23-94`)

Pub/Sub accumulator — like a message queue:

```python
# channels/topic.py:23
class Topic(Generic[Value], BaseChannel[Sequence[Value], Value | list[Value], list[Value]]):
    """A configurable PubSub Topic.
    accumulate: If False, the channel is emptied after each step.
                If True, values accumulate across steps."""

    def __init__(self, typ: type[Value], accumulate: bool = False):
        self.accumulate = accumulate
        self.values = list[Value]()

    def update(self, values: Sequence[Value | list[Value]]) -> bool:
        if not self.accumulate:
            self.values = list[Value]()  # Clear if not accumulating
        self.values.extend(flatten(values))
        return True
```

### Channel Catalog

| Channel | ValueType | UpdateType | Checkpoint | Use Case |
|---------|-----------|------------|------------|----------|
| `LastValue` | `T` | `T` | `T` | Single-value state fields |
| `LastValueAfterFinish` | `T` | `T` | `(T, bool)` | Output available only after graph finishes |
| `Topic` | `Sequence[T]` | `T | list[T]` | `list[T]` | Accumulating outputs, pub/sub |
| `EphemeralValue` | `T` | `T` | `MISSING` | Triggers without persisting |
| `BinaryOperatorAggregate` | `T` | `T` | `T` | Running totals,Reducers |
| `AnyValue` | `T` | `T` | `T` | Accepts any single update |
| `UntrackedValue` | `T` | `T` | `MISSING` | Values not saved to checkpoints |
| `NamedBarrierValue` | `str` | `str` | `set[str]` | Synchronization across N named parties |

## StateGraph — The High-Level API (`graph/state.py`)

`StateGraph` is the user-facing API that compiles to `Pregel`:

```python
# graph/state.py:89
__all__ = ("StateGraph", "CompiledStateGraph")
```

How it works:
1. User defines a `TypedDict` or `Pydantic` model as state schema
2. Each field becomes a channel (default: `LastValue`)
3. `Annotated[Type, reducer]` fields use custom reducers (e.g., `add_messages`)
4. `add_node(name, func)` creates `PregelNode`s
5. `add_edge(src, dst)` creates `ChannelWrite` entries
6. `add_conditional_edges(src, condition, mapping)` creates dynamic branches
7. `compile()` produces a `CompiledStateGraph` wrapping a `Pregel` instance

Schema-to-channels mapping (conceptual):

```python
class MyState(TypedDict):
    messages: Annotated[list[BaseMessage], add_messages]  # → Topic channel with add_messages reducer
    status: str                                           # → LastValue channel
    metadata: dict                                        # → LastValue channel
```

## Functional API (`func/__init__.py`)

`@entrypoint` and `@task` — imperative style:

```python
# func/__init__.py:44
__all__ = ("task", "entrypoint")

@task(
    retry_policy=RetryPolicy(max_attempts=3),
    cache_policy=CachePolicy(key_function=...),
)
def my_task(input: str) -> str:
    ...

@entrypoint(checkpointer=MemorySaver())
def my_workflow(input: str) -> str:
    result = my_task(input).result()  # .result() waits for the future
    return result
```

Under the hood, `@entrypoint` compiles to a `Pregel` instance with:
- `entrypoint` function → root `PregelNode`
- `@task` calls → dynamically created `PregelNode`s
- `call()` → creates `PregelExecutableTask` with task-specific config

## Checkpointing System

### Checkpoint Structure (`checkpoint/base/__init__.py:65-97`)

```python
# checkpoint/base/__init__.py:65
class Checkpoint(TypedDict):
    """State snapshot at a given point in time."""
    v: int                                    # Version (currently 1)
    id: str                                   # UUID6, monotonically increasing
    ts: str                                   # ISO 8601 timestamp
    channel_values: dict[str, Any]            # Current channel values
    channel_versions: ChannelVersions          # {channel: version} for scheduling
    versions_seen: dict[str, ChannelVersions]  # {node: {channel: version}}
    updated_channels: list[str] | None         # Channels updated in this step
```

```python
# checkpoint/base/__init__.py:35
class CheckpointMetadata(TypedDict, total=False):
    source: Literal["input", "loop", "update", "fork"]
    step: int                                  # -1 for input, 0 for first loop
    parents: dict[str, str]                    # Parent checkpoint IDs by namespace
    run_id: str                                # Run that created this checkpoint
```

```python
# checkpoint/base/__init__.py:100+
class CheckpointTuple(NamedTuple):
    config: RunnableConfig
    checkpoint: Checkpoint
    metadata: CheckpointMetadata
    parent_config: Optional[RunnableConfig]
    pending_writes: Optional[list[PendingWrite]]
```

### Checkpointer Interface

```python
# checkpoint/base/__init__.py
class BaseCheckpointSaver(ABC):
    def put(self, config, checkpoint, metadata) -> RunnableConfig: ...
    def get_tuple(self, config) -> CheckpointTuple | None: ...
    def list(self, config, *, limit=None, before=None, filter=None) -> Iterator[CheckpointTuple]: ...
    def put_writes(self, config, writes, task_id) -> None: ...
```

### Serializer (`checkpoint/serde/jsonplus.py:67`)

```python
# checkpoint/serde/jsonplus.py:67
class JsonPlusSerializer(SerializerProtocol):
    """Serializer that uses ormsgpack, with optional fallbacks.

    Security note: This serializer should not be used on untrusted python objects.
    If an attacker can write directly to your checkpoint database,
    they may be able to trigger code execution when data is deserialized.

    Set LANGGRAPH_STRICT_MSGPACK=true to restrict to a built-in allowlist of safe types.
    """
```

Uses `ormsgpack` for fast serialization with:
- Custom type handlers for Pydantic models, dataclasses, datetime, UUID, etc.
- Pickle fallback for unsupported types (configurable)
- Strict mode (`LANGGRAPH_STRICT_MSGPACK=true`) to block arbitrary Python types

### Persistence Backends

| Backend | Location | Type |
|---------|----------|------|
| `MemorySaver` | `checkpoint/memory/` | In-memory dict |
| `SqliteSaver` | `checkpoint-sqlite/` | SQLite file |
| `PostgresSaver` | `checkpoint-postgres/` | PostgreSQL |
| `RedisCache` | `checkpoint/cache/redis/` | Redis (cache only) |

## Command, Send, and Control Flow

### Command (`types.py`)

```python
# types.py (conceptual)
@dataclass
class Command:
    """Control flow primitive for dynamic graphs."""
    goto: str | Send | Sequence[str | Send] | None  # Where to go next
    update: dict[str, Any] | None                    # State updates to apply
    graph: str | None                                # Target subgraph
```

`Command` replaces static edges:
```python
def router_node(state):
    if should_call_tool:
        return Command(goto="tool_node")
    elif should_ask_human:
        return Command(goto="human_review")
    else:
        return Command(goto=END)
```

### Send (`types.py`)

```python
# types.py
@dataclass
class Send:
    """Send data to a specific node dynamically."""
    node: str
    arg: Any
```

Used for fan-out patterns:
```python
def parallel_node(state):
    return Command(goto=[
        Send("worker", task_id=1),
        Send("worker", task_id=2),
        Send("worker", task_id=3),
    ])
```

### Interrupt (`types.py`)

```python
# types.py
def interrupt(value: Any) -> Any:
    """Pause execution and wait for human input.
    The value is shown to the human, their response replaces the interrupt."""
```

## Durability Modes (`types.py:85-91`)

```python
Durability = Literal["sync", "async", "exit"]

# "sync"  — checkpoint is saved before next step starts (safest, slowest)
# "async" — checkpoint saved in background while next step runs (fastest, risk of losing last step on crash)
# "exit"  — checkpoint only saved when graph exits (fastest, no crash recovery during execution)
```

This is enforced in `PregelLoop.put_writes()` (`_loop.py:394`):
```python
if self.durability != "exit" and self.checkpointer_put_writes is not None:
    self.submit(self.checkpointer_put_writes, config, writes, task_id, ...)
    # "async" submits to background executor
    # "sync" would wait for the future to complete
```

## Runtime and Execution Context (`runtime.py`)

```python
# runtime.py:90
@dataclass(frozen=True, slots=True)
class Runtime(Generic[ContextT]):
    """Injected into graph nodes. Provides access to context, store, stream_writer,
    previous, and execution_info."""

    context: ContextT                      # User-defined context (dependency injection)
    store: BaseStore | None                # Long-term memory across threads
    stream_writer: StreamWriter            # Write to custom stream
    previous: Any                          # Previous state (for resumable workflows)
    execution_info: ExecutionInfo          # Read-only metadata
    server_info: ServerInfo | None         # LangGraph Server metadata

# runtime.py:24
@dataclass(frozen=True, slots=True)
class ExecutionInfo:
    checkpoint_id: str
    checkpoint_ns: str
    task_id: str
    thread_id: str | None
    run_id: str | None
    node_attempt: int                      # Current retry attempt (1-indexed)
    node_first_attempt_time: float | None  # When first retry started
```

Access from nodes:
```python
from langgraph.runtime import get_runtime

def my_node(state, runtime: Runtime) -> dict:
    user = runtime.server_info.user  # Authenticated user
    thread = runtime.execution_info.thread_id
    runtime.store.search(("user_data",), query="...")
    return state
```

## Stream Modes (`types.py:118-132`)

```python
StreamMode = Literal["values", "updates", "checkpoints", "tasks", "debug", "messages", "custom"]
```

| Mode | What's Emitted | When |
|------|---------------|------|
| `"values"` | Full state after each step | After `apply_writes` |
| `"updates"` | Node name + delta returned | After each node finishes |
| `"checkpoints"` | Checkpoint event | When checkpoint is saved |
| `"tasks"` | Task start/finish events | When tasks are scheduled/completed |
| `"debug"` | Both checkpoints + task events | For debugging |
| `"messages"` | LLM tokens + metadata | Token-by-token from LLM calls |
| `"custom"` | User-defined via `StreamWriter` | When `stream_writer(value)` is called |

## Retry and Cache Policies

### RetryPolicy (`types.py`)

```python
# types.py (conceptual)
@dataclass
class RetryPolicy:
    initial_interval: float = 0.5
    max_interval: float = 128.0
    max_attempts: int = 3
    exponential_base: float = 2.0
    jitter: bool = True
    retry_on: Callable[[Exception], bool] = default_retry_on
```

Applied per-node:
```python
graph.add_node("flaky_api", call_api, retry_policy=[
    RetryPolicy(max_attempts=3, retry_on=lambda e: isinstance(e, ConnectionError))
])
```

### CachePolicy (`types.py`)

```python
# types.py (conceptual)
@dataclass
class CachePolicy:
    key_function: Callable[..., str | bytes] = default_cache_key
    ttl: float | None = None  # No expiry if None
```

Cached task results are reused — skipped entirely if the cache key matches:

```python
@task(cache_policy=CachePolicy(ttl=3600))  # Cache for 1 hour
def expensive_computation(input: str) -> str: ...
```

Cache key is computed from function identifier + input hash (xxhash3 128-bit).

## Prebuilt Agents (`prebuilt/`)

### Chat Agent Executor (`prebuilt/chat_agent_executor.py`)

The canonical ReAct agent:

```python
# prebuilt/chat_agent_executor.py
def create_react_agent(
    model: BaseChatModel,
    tools: Sequence[BaseTool] | ToolNode,
    *,
    checkpointer: Checkpointer = None,
    prompt: BaseChatPromptTemplate | None = None,
    ...
) -> CompiledStateGraph:
    """Create a ReAct agent with tool calling."""
```

State schema:
```python
class AgentState(TypedDict):
    messages: Annotated[Sequence[BaseMessage], add_messages]
    remaining_steps: NotRequired[RemainingSteps]  # Recursion guard
```

Graph structure:
```
START → agent (LLM with tools) → tools (ToolNode) → agent → ... → END
                              ↓
                         (no tool_calls) → END
```

### ToolNode (`prebuilt/tool_node.py`)

Executes tool calls from `AIMessage`:

```python
class ToolNode(RunnableCallable):
    """Executes tools specified in AIMessage.tool_calls.
    Handles errors, retries, and tool output formatting."""
```

## Key Internal Mechanisms

### Dotted Order and Checkpoint Namespaces

```python
# _internal/_constants.py
CONFIG_KEY_CHECKPOINT_NS = "checkpoint_ns"    # Namespace for checkpoint isolation
CONFIG_KEY_THREAD_ID = "thread_id"             # Thread for checkpoint grouping
CONFIG_KEY_CHECKPOINT_ID = "checkpoint_id"     # Specific checkpoint to resume from
```

Checkpoint namespace enables:
- Subgraph checkpoint isolation (each subgraph gets its own namespace)
- Parallel thread execution (each thread has its own checkpoints)
- Fork/resume from any point in history

### Scratchpad (`_internal/_scratchpad.py`)

Per-execution mutable state that doesn't get checkpointed:

```python
# _internal/_scratchpad.py
class PregelScratchpad:
    """Temporary storage for execution metadata."""
    subgraph_counter: Callable[[], int]
    # ... other per-run counters
```

### Apply Writes (`pregel/_algo.py`)

The synchronization barrier:

```python
# pregel/_algo.py (conceptual)
def apply_writes(checkpoint, channels, tasks, get_next_version, trigger_to_nodes):
    """Apply all task outputs to channels atomically.
    Returns the set of channels that were updated."""

    # 1. Group writes by channel
    writes_by_channel = defaultdict(list)
    for task in tasks:
        for channel, value in task.writes:
            writes_by_channel[channel].append(value)

    # 2. Update each channel
    updated = set()
    for channel, values in writes_by_channel.items():
        if channels[channel].update(values):
            updated.add(channel)
            checkpoint["channel_versions"][channel] = get_next_version()

    # 3. Update versions_seen for completed tasks
    for task in tasks:
        checkpoint["versions_seen"][task.id] = {
            ch: checkpoint["channel_versions"][ch]
            for ch in channels  # Node has now seen all current channel versions
        }

    return updated
```

## Architecture Summary

```
┌─────────────────────────────────────────────────────────────┐
│                    User Code                                 │
│  StateGraph / @entrypoint / CompiledStateGraph               │
└────────────────────┬────────────────────────────────────────┘
                     │ compile()
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                    Pregel Runtime                            │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  PregelLoop.tick() — BSP Superstep                   │    │
│  │                                                     │    │
│  │  1. PLAN: prepare_next_tasks()                      │    │
│  │     → versions_seen determines which nodes run      │    │
│  │     → trigger_to_nodes maps channel→nodes           │    │
│  │                                                     │    │
│  │  2. EXECUTE: PregelRunner.submit()                  │    │
│  │     → BackgroundExecutor runs tasks in parallel     │    │
│  │     → RetryPolicy on failure                        │    │
│  │     → CachePolicy on hit                            │    │
│  │                                                     │    │
│  │  3. UPDATE: apply_writes()                          │    │
│  │     → Channel.update(values) for each channel       │    │
│  │     → channel_versions incremented                  │    │
│  │     → versions_seen updated for completed nodes     │    │
│  │     → checkpoint saved (durability: sync/async/exit)│    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  Channels: LastValue | Topic | EphemeralValue | BinOp      │
│  Control: Command | Send | Interrupt                        │
│  Context: Runtime | ExecutionInfo | BaseStore               │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                    Persistence Layer                         │
│  Checkpointer: MemorySaver | SqliteSaver | PostgresSaver    │
│  Serializer: JsonPlusSerializer (ormsgpack + pickle fallback)│
│  Store: InMemoryStore | PostgresStore                        │
│  Cache: InMemoryCache | RedisCache                           │
└─────────────────────────────────────────────────────────────┘
```

## Key Takeaways

1. **Pregel is a concrete class** (`main.py:343`), not an abstract pattern. `StateGraph.compile()` returns `CompiledStateGraph` which wraps a `Pregel` instance.

2. **Channels are the state machine** — not dicts. Each state field maps to a `BaseChannel` instance with typed `get()`/`update()`/`checkpoint()` methods. `LastValue` is the default, `Topic` for accumulation, `BinaryOperatorAggregate` for reducers.

3. **`versions_seen` is the scheduler** — nodes run when their subscribed channels have newer versions than what the node has seen. This is the Pregel "plan" phase, implemented in `prepare_next_tasks()`.

4. **Durability is per-invocation** — `Durability = "sync" | "async" | "exit"` controls when checkpoints are saved. Trade latency for crash safety per run.

5. **`Command` replaces static edges** — dynamic control flow via `Command(goto=...)`, `Send(node, data)`, and `interrupt()`. The agent decides the next node at runtime.

6. **Serialization uses ormsgpack** (`JsonPlusSerializer`) with pickle fallback and strict mode (`LANGGRAPH_STRICT_MSGPACK=true`) for security.

7. **`@entrypoint` and `StateGraph` compile to the same Pregel** — the functional API creates the same `PregelNode`s and channels under the hood.

[Back to core principles overview → 00-overview.md](00-overview.md)
[See LangSmith internals → ../agent-observability/01-langsmith-internals.md](../agent-observability/01-langsmith-internals.md)
