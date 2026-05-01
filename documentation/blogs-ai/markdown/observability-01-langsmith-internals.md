---
title: LangSmith Internals -- Source Code Deep Dive
---

# LangSmith Internals -- Source Code Deep Dive

## Purpose

This document reverse-engineers the LangSmith Python SDK (v0.8.0) from source code to understand how tracing, evaluation, and the client architecture actually work under the hood. All code references are from the installed package.

Source: `pip install langsmith==0.8.0` — introspected at runtime.

## Aha Moments

**Aha: Tracing uses `contextvars` for parent-child linking, not global state.** The SDK uses Python's `contextvars` module (not `threading.local`) to track the current parent run. This means nested async functions, threads, and coroutines each get their own tracing context automatically — no manual parent wiring needed.

**Aha: Traces are batched in a background thread with ZSTD compression.** The SDK doesn't send traces synchronously. It queues them in a `PriorityQueue`, compresses with ZSTD, and flushes in a background daemon thread. This is why `@traceable` has zero latency impact on the hot path.

**Aha: The `dotted_order` string is the real trace tree.** Instead of querying parent-child relationships in the database, LangSmith encodes the entire tree structure as a sortable dotted string: `{timestamp}{uuid}.{timestamp}{uuid}.child...`. This enables O(1) tree reconstruction without recursive queries.

**Aha: UUIDs are UUIDv7 (time-sortable), not UUIDv4 (random).** Time-sortable UUIDs mean traces are naturally ordered in the database without needing a separate index on `start_time`. The SDK includes its own `uuid7` implementation.

**Aha: Write replicas let you fan out traces to multiple backends.** A single `RunTree` can be configured with multiple `WriteReplica` entries, each pointing to a different API endpoint with different auth. This enables sending traces to both LangSmith cloud and a self-hosted instance simultaneously.

**Aha: The `@traceable` decorator supports seven function signatures: sync, async, generators, async generators, and context managers.** It inspects the function at decoration time and generates the correct wrapper for each case.

## Public API Surface

```python
# langsmith.__init__.py — 24 public symbols
from langsmith import (
    # Tracing
    traceable,           # Decorator for auto-capture
    trace,               # Context manager for manual tracing
    tracing_context,     # Block-scoped tracing config
    get_tracing_context, # Read current tracing context
    get_current_run_tree, # Get the active RunTree
    set_run_metadata,    # Add metadata to current run
    RunTree,             # Run schema with back-references
    configure,           # Global tracing configuration

    # Client
    Client,              # Sync API client
    AsyncClient,         # Async API client
    TracingMode,         # "normal" | "otel" | "disabled"

    # Evaluation
    evaluate,            # Run evals on a function
    evaluate_existing,   # Run evals on existing dataset
    aevaluate,           # Async evaluate
    aevaluate_existing,  # Async evaluate existing
    EvaluationResult,    # Single evaluation result
    RunEvaluator,        # Base class for evaluators

    # Testing
    test,                # Test decorator
    expect,              # Assertion helper
    unit,                # Unit test marker

    # Prompt caching
    PromptCache,         # Prompt cache
    AsyncPromptCache,    # Async prompt cache
    configure_global_prompt_cache,

    # Utilities
    ContextThreadPoolExecutor,  # Thread pool that propagates contextvars
    uuid7, uuid7_from_datetime, # Time-sortable UUIDs
    set_runtime_overrides,     # Override runtime parameters
)
```

## Core Data Model (from `schemas.py`)

### RunBase — The Fundamental Unit

```python
# schemas.py:306-390
class RunBase(BaseModel):
    id: UUID                        # UUIDv7, time-sortable
    name: str                       # Human-readable name
    start_time: datetime            # When the run started
    run_type: str                   # "llm" | "tool" | "chain" | "retriever" | "embedding" | "prompt" | "parser"
    end_time: Optional[datetime]
    extra: Optional[dict]           # Contains metadata, callbacks
    error: Optional[str]            # Error message if failed
    serialized: Optional[dict]      # Serialized object that executed
    events: Optional[list[dict]]    # Start/end events, streaming chunks
    inputs: dict                    # Input data
    outputs: Optional[dict]         # Output data
    reference_example_id: Optional[UUID]  # Dataset example this run is based on
    parent_run_id: Optional[UUID]   # Parent for nesting
    tags: Optional[list[str]]       # Categorization tags
    attachments: dict               # Binary attachments (mime_type, bytes)

    @property
    def metadata(self) -> dict[str, Any]:  # Convenience: extra.setdefault("metadata", {})
    @property
    def latency(self) -> Optional[float]:   # end_time - start_time in seconds
```

The `run_type` field is the key discriminator — it determines how the run is displayed and evaluated in LangSmith.

### Run — Database Schema Extension

```python
# schemas.py:393-455
class Run(RunBase):
    session_id: Optional[UUID]          # Project ID
    child_run_ids: Optional[list[UUID]] # Deprecated
    child_runs: Optional[list[Run]]     # Loaded on demand
    feedback_stats: Optional[dict]
    manifest_id: Optional[UUID]         # Serialized object ID
    status: Optional[str]               # "success" | "error" | "partial"
    prompt_tokens: Optional[int]
    completion_tokens: Optional[int]
    total_tokens: Optional[int]
    prompt_token_details: Optional[dict[str, int]]    # cache_read, cache_creation, audio, reasoning
    completion_token_details: Optional[dict[str, int]] # cache_read, audio, reasoning
    first_token_time: Optional[datetime] # TTFT
    total_cost: Optional[Decimal]
    prompt_cost: Optional[Decimal]
    completion_cost: Optional[Decimal]
    trace_id: UUID                      # Root run ID of this trace
    dotted_order: str                   # Tree position encoding
```

### The `dotted_order` Encoding

```python
# run_trees.py — the dotted_order format
# Example: "20260501T103000000000Zabc123.20260501T103000001000Zdef456"
#            └───── timestamp ─────┘└─ uuid ─┘ └───── timestamp ─────┘└─ uuid ─┘
#            root run                        child run

# This enables:
# 1. Lexicographic sort = chronological sort (timestamps are ISO-8601 with fixed width)
# 2. Tree reconstruction by splitting on "." and counting levels
# 3. No recursive SQL queries needed
```

## Tracing Engine (from `run_helpers.py` and `run_trees.py`)

### Context Variable Architecture

```python
# run_helpers.py:61-70
_CONTEXT_KEYS: dict[str, contextvars.ContextVar] = {
    "parent_ref": _context._PARENT_RUN_TREE_REF,    # WeakRef[RunTree] — current parent
    "project_name": _context._PROJECT_NAME,          # str — which project to log to
    "tags": _context._TAGS,                          # list[str] — global tags
    "metadata": _context._METADATA,                  # dict — global metadata
    "enabled": _context._TRACING_ENABLED,            # bool | "local"
    "client": _context._CLIENT,                      # Client instance
    "replicas": run_trees._REPLICAS,                # list[WriteReplica]
    "distributed_parent_id": run_trees._DISTRIBUTED_PARENT_ID,  # for distributed tracing
}
```

The use of `contextvars` (not `threading.local`) means:
- Async coroutines get their own tracing context
- `asyncio.create_task()` copies the parent context
- Thread pools need `ContextThreadPoolExecutor` to propagate context

### The `@traceable` Decorator — How It Works

```python
# run_helpers.py:330-450 (simplified)
@overload
def traceable(
    run_type: str = "chain",
    *,
    name: Optional[str] = None,
    metadata: Optional[Mapping] = None,
    tags: Optional[list[str]] = None,
    client: Optional[Client] = None,
    reduce_fn: Optional[Callable] = None,    # For streaming: merge chunks
    project_name: Optional[str] = None,
    process_inputs: Optional[Callable] = None,  # Sanitize inputs before sending
    process_outputs: Optional[Callable] = None, # Sanitize outputs before sending
    process_chunk: Optional[Callable] = None,   # Process streaming chunks
    _invocation_params_fn: Optional[Callable] = None,
    dangerously_allow_filesystem: bool = False,  # Allow file attachments
) -> Callable: ...
```

The decorator flow:
1. **Function inspection**: At decoration time, inspects if the function is sync/async/generator
2. **RunTree creation**: On call, creates a child `RunTree` from the current context parent
3. **Context push**: Sets the new RunTree as the active parent via `contextvars`
4. **Execution**: Runs the function, captures inputs/outputs/errors
5. **Context pop**: Resets the context to the previous parent
6. **Async flush**: Enqueues the RunTree for background sending

### The `trace` Context Manager

```python
# run_helpers.py
@contextmanager
def trace(
    name: str,
    run_type: str = "chain",
    inputs: Optional[dict] = None,
    tags: Optional[list[str]] = None,
    metadata: Optional[dict] = None,
    project_name: Optional[str] = None,
    client: Optional[Client] = None,
) -> Generator[RunTree, None, None]:
    ...

# Usage:
with trace("agent_loop", run_type="chain", inputs={"goal": "fix bug"}) as run:
    # ... do work
    run.end(outputs={"result": "fixed"})
```

### The `tracing_context` Manager

```python
# run_helpers.py:142-210
with tracing_context(project_name="experiment_v2", tags=["production"]):
    # All @traceable calls inside this block inherit these settings
    my_agent.run()
```

Key feature: it saves and restores the previous context, so it's composable and stackable.

### RunTree — The Core Object

```python
# run_trees.py:249-290
class RunTree(RunBase):
    name: str
    id: UUID = Field(default_factory=uuid7)           # UUIDv7
    run_type: str = Field(default="chain")
    start_time: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))
    parent_run: Optional[RunTree] = Field(default=None, exclude=True)
    parent_dotted_order: Optional[str] = Field(default=None, exclude=True)
    child_runs: list[RunTree] = Field(default_factory=list, exclude=True)
    session_name: str                                  # Project name
    dotted_order: str = Field(default="")              # Tree position
    trace_id: UUID = Field(default="")                 # Root run ID
    replicas: Optional[Sequence[WriteReplica]]         # Fan-out targets

    def post(self, exclude_child_runs: bool = False) -> None:
        """Send this run to the LangSmith API."""
        ...

    def patch(self) -> None:
        """Update an existing run (for streaming updates)."""
        ...
```

### Distributed Tracing

LangSmith supports distributed tracing via HTTP headers. When a RunTree is serialized for cross-process communication:

```python
# run_trees.py — distributed tracing headers
LANGSMITH_PREFIX = "langsmith-"
LANGSMITH_DOTTED_ORDER = "langsmith-trace"        # Carries the dotted_order string
LANGSMITH_METADATA = "langsmith-metadata"          # Carries metadata JSON
LANGSMITH_TAGS = "langsmith-tags"                  # Carries tags as JSON
LANGSMITH_PROJECT = "langsmith-project"            # Carries project name
LANGSMITH_REPLICAS = "langsmith-replicas"          # Carries replica config

# To propagate across processes:
headers = run_tree.to_headers()
# Then on the receiving end:
child = RunTree.from_headers(headers, name="cross_process_call")
```

## Write Replicas — Multi-Backend Tracing

```python
# run_trees.py:53-73
class WriteReplica(TypedDict, total=False):
    api_url: Optional[str]          # LangSmith API URL
    auth: AuthHeaders               # Auth (api_key, service_key, JWT)
    project_name: Optional[str]     # Which project to write to
    updates: Optional[dict]         # Mutations to apply before sending
    client: Optional[Client]        # Dedicated client (can use different tracing_mode)
```

Use case: send traces to both production and experiment projects simultaneously.

```python
ls.configure(
    replicas=[
        {"project_name": "production", "updates": {"tags": ["prod"]}},
        {"project_name": "experiment_v2", "updates": {"tags": ["exp"]}},
    ],
)
```

## Background Thread — How Traces Get Sent

```python
# _background_thread.py — TracingQueueItem
@functools.total_ordering
class TracingQueueItem:
    priority: str                     # "create" or "patch"
    item: SerializedRunOperation | SerializedFeedbackOperation
    api_url: Optional[str]
    api_key: Optional[str]
    # ... auth fields for replica routing
```

### The Batching Pipeline

```
@traceable decorated function
    ↓
RunTree created, enqueued in PriorityQueue
    ↓
Background thread wakes up (every N seconds or when queue is full)
    ↓
Groups items by (api_url, api_key) — batches by endpoint+auth
    ↓
Compresses with ZSTD (zstandard library)
    ↓
Sends multipart POST to /runs endpoint
    ↓
On failure: retries with exponential backoff
```

Key constants:
```python
# _constants.py
_AUTO_SCALE_UP_NTHREADS_LIMIT = 16   # Max sender threads
_AUTO_SCALE_UP_QSIZE_TRIGGER = 30000  # Scale up when queue > 30K
_AUTO_SCALE_DOWN_NEMPTY_TRIGGER = 100 # Scale down after 100 empty polls
```

The queue auto-scales the number of sender threads based on queue pressure — more threads when the queue grows, fewer when it drains.

## Provider Wrappers — Auto-Tracing for LLM APIs

### Anthropic Wrapper (`wrappers/_anthropic.py`)

```python
# _anthropic.py:62-97
def _infer_ls_params(prepopulated_invocation_params, kwargs):
    stripped = _strip_not_given(kwargs)
    return {
        "ls_provider": "anthropic",
        "ls_model_type": "chat",
        "ls_model_name": stripped.get("model"),
        "ls_temperature": stripped.get("temperature"),
        "ls_max_tokens": stripped.get("max_tokens"),
        "ls_stop": stop,
        "ls_invocation_params": {...},  # mcp_servers, tool_choice, top_k, top_p, thinking
    }
```

The wrapper:
1. Patches `Anthropic.messages.create` to auto-trace every call
2. Strips `NotGiven` sentinel values from the Anthropic SDK
3. Extracts usage metadata (token counts, costs)
4. Supports streaming — processes each chunk as a run event
5. Handles `thinking` blocks for Claude 3.7+

### OpenAI, Gemini, OpenAI Agents wrappers

Similar pattern for each provider — the wrapper intercepts the client's method, calls the original, and traces the inputs/outputs with provider-specific metadata extraction.

## Evaluation Engine (from `evaluation/`)

### Core Primitives

```python
# evaluator.py
class EvaluationResult(BaseModel):
    key: str                    # Metric name, e.g., "accuracy", "helpfulness"
    score: Union[bool, int, float, None]
    value: Union[dict, str, bool, int, float, None]  # Additional detail
    comment: Optional[str]      # Human-readable explanation

class RunEvaluator(Protocol):
    def evaluate_run(self, run: Run, *, example: Optional[Example] = None) -> EvaluationResult:
        ...
```

### Evaluation Runner

```python
# _runner.py
def evaluate(
    func: Callable,                    # Function to evaluate
    data: InputDataT,                  # Dataset (list of dicts, dataset ID, etc.)
    evaluators: Optional[list[RunEvaluatorFactory]] = None,
    summary_evaluators: Optional[list[Callable]] = None,
    num_repetitions: int = 1,
    concurrency_level: int = 1,
    experiment: Optional[str] = None,
    metadata: Optional[dict] = None,
) -> ExperimentResults:
    ...
```

The evaluation flow:
1. Load dataset examples
2. Run the function on each example (with tracing enabled)
3. Run evaluators on the resulting runs
4. Aggregate scores and upload to LangSmith
5. Return `ExperimentResults` for programmatic access

## Client Architecture (from `client.py`)

### The Client

```python
# client.py
class Client:
    def __init__(
        self,
        api_key: Optional[str] = None,
        api_url: Optional[str] = None,
        timeout_ms: int = 10000,
        auto_batch_tracing: bool = True,
        hide_inputs: Optional[Callable] = None,
        hide_outputs: Optional[Callable] = None,
        tracing_mode: Optional[str] = None,  # "normal" | "otel" | "disabled"
    ):
        ...
```

Key features:
- **Auto-batching**: Runs are batched and sent in the background
- **Input/output masking**: `hide_inputs` and `hide_outputs` callbacks can redact sensitive data
- **OTel support**: `tracing_mode="otel"` sends traces via OpenTelemetry SDK
- **Retry logic**: Uses `urllib3.util.Retry` with exponential backoff
- **Connection pooling**: Uses `requests.adapters.HTTPAdapter` with pool management

## Testing Integration

### `@test` and `expect`

```python
# testing/_internal.py (conceptual)
from langsmith import test, expect

@test
def test_agent_handles_empty_input():
    result = my_agent("")
    expect(result).to_have("error_message")
    expect(result.status).to_equal("failed")
```

The `@test` decorator wraps test functions with tracing, and `expect` provides assertion-style API that records pass/fail as feedback in LangSmith.

### Pytest Plugin

```python
# pytest_plugin.py
# Adds --langsmith-output flag for rich test output
# Integrates with pytest to display LangSmith test results inline
```

## Prompt Cache

```python
# prompt_cache.py
class PromptCache:
    """Cache prompts to avoid regenerating identical ones."""

    def get(self, prompt: str) -> Optional[str]:
        """Return cached response if prompt matches."""

    def set(self, prompt: str, response: str):
        """Cache a prompt-response pair."""
```

Useful for deterministic prompts or during development iterations.

## Key Implementation Details

### UUIDv7 — Time-Sortable Identifiers

```python
# uuid.py
def uuid7() -> UUID:
    """Generate a UUIDv7 — time-sortable UUID."""
    # First 48 bits = Unix timestamp in milliseconds
    # Remaining 80 bits = random
    # This means UUIDs sort chronologically when compared as strings

def uuid7_from_datetime(dt: datetime) -> UUID:
    """Generate a deterministic UUIDv7 from a specific datetime."""
    # Used for reconstructing UUIDs from start_time (e.g., when replaying traces)
```

### Context Propagation in Thread Pools

```python
# utils.py
class ContextThreadPoolExecutor(concurrent.futures.ThreadPoolExecutor):
    """ThreadPoolExecutor that propagates contextvars to worker threads."""

    def submit(self, fn, *args, **kwargs):
        ctx = contextvars.copy_context()
        return super().submit(ctx.run, functools.partial(fn, *args, **kwargs))
```

Without this, `@traceable` in a `ThreadPoolExecutor` would lose the parent-child relationship because `contextvars` are thread-local by default.

### Serialization Safety

```python
# _internal/_serde.py
# Uses orjson for fast JSON serialization
# Handles datetime, UUID, bytes, and Pydantic models
# Falls back to standard json for edge cases
```

## Architecture Summary

```
┌─────────────────────────────────────────────────────────────┐
│                      Your Agent Code                         │
│  @traceable functions, wrapped LLM clients                   │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                    Tracing Layer                             │
│  contextvars → parent-child linking                          │
│  RunTree → run schema with back-references                  │
│  UUIDv7 → time-sortable identifiers                          │
│  dotted_order → tree encoding without recursive queries      │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                  Background Dispatcher                       │
│  PriorityQueue → ordered by create-before-patch              │
│  ZSTD compression → reduces payload size                     │
│  Auto-scaling threads → 1 to 16 senders based on pressure   │
│  Group by endpoint → batch by (api_url, api_key) combo       │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                    LangSmith API                             │
│  /runs → create/update runs                                  │
│  /feedback → record evaluation scores                        │
│  /datasets → manage test data                                │
│  /comparative → A/B test experiments                         │
└─────────────────────────────────────────────────────────────┘
```

## Key Takeaways

1. **Context-driven architecture**: `contextvars` is the backbone — no manual parent wiring, works with async and threads (with `ContextThreadPoolExecutor`).

2. **Zero-latency tracing**: Background thread + queue + ZSTD compression means `@traceable` adds sub-millisecond overhead to the hot path.

3. **Dotted order is the tree**: No recursive queries needed — the dotted string encodes the full parent-child hierarchy in a sortable format.

4. **Provider-agnostic**: Wrappers for Anthropic, OpenAI, Gemini, and OpenAI Agents extract provider-specific metadata into a unified schema.

5. **Multi-backend capable**: Write replicas enable fan-out to multiple LangSmith instances or projects simultaneously.

6. **OTel integration**: `tracing_mode="otel"` sends traces via OpenTelemetry for vendor-neutral observability.

[Back to observability guide → 00-overview.md](00-overview.md)
