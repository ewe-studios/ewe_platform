# hermes-agent Tool System

## File Locations

- Registry: `tools/registry.py`
- Tool orchestration: `model_tools.py`
- Toolsets: `toolsets.py`
- Tool execution: `run_agent.py` (`_execute_tool_calls` at line ~6029)
- Individual tools: `tools/*.py` (50+ files)

## Registry Pattern (`tools/registry.py`)

The `ToolRegistry` is a singleton where each tool self-registers at import time:

```python
registry.register(
    name="tool_name",
    toolset="core",
    schema={...},          # OpenAI function format
    handler=callable,      # Sync or async function
    check_fn=callable,     # Availability check
    requires_env=["ENV_VAR"],
    is_async=True,
    description="Tool description",
    emoji="📁",
)
```

The registry provides:
- `get_definitions(tool_names)` -- Returns OpenAI-format tool schemas (only tools whose `check_fn()` passes)
- `dispatch(name, args, **kwargs)` -- Executes handler, bridges async handlers, catches exceptions
- `get_all_tool_names()`, `get_schema()`, `get_toolset_for_tool()`, `is_toolset_available()`

## Tool Discovery (`model_tools.py._discover_tools()`)

The discovery process:
1. Imports 20+ tool modules to trigger their `register()` calls
2. Discovers MCP tools (Model Context Protocol)
3. Discovers plugin tools
4. Returns the combined set of registered tools

## Toolset System (`toolsets.py`)

`_HERMES_CORE_TOOLS` is the shared tool list for all platforms. Toolsets can:
- List direct tools
- Include other toolsets (recursively resolved with cycle detection)
- Define platform-specific variants

| Toolset | Purpose |
|---------|---------|
| `core` | Shared tools for all platforms |
| `hermes-cli` | CLI-specific tools |
| `hermes-telegram` | Telegram-specific tools |
| `hermes-acp` | ACP (IDE integration) tools |
| ... | Other platform-specific toolsets |

All platform toolsets reference `_HERMES_CORE_TOOLS` as their base.

## Argument Coercion (`model_tools.py.coerce_tool_args()`)

LLMs frequently return typed values as strings. The coercion function:

```python
def coerce_tool_args(tool_name, args):
    """Coerce tool call arguments to match their JSON Schema types."""
    schema = registry.get_schema(tool_name)
    properties = (schema.get("parameters") or {}).get("properties")
    for key, value in args.items():
        if not isinstance(value, str):
            continue
        prop_schema = properties.get(key)
        expected = prop_schema.get("type")  # "integer", "number", "boolean"
        coerced = _coerce_value(value, expected)
        if coerced is not value:
            args[key] = coerced
    return args
```

Coercion helpers:
- `_coerce_number()`: parses `"42"` -> `42`, `"3.14"` -> `3.14`
- `_coerce_boolean()`: parses `"true"/"True"/"yes"` -> `true`, `"false"/"False"/"no"` -> `false`
- Union types (e.g., `["integer", "string"]`): tries each type in order

## Tool Execution (`_execute_tool_calls` at line ~6029)

### Parallel vs Sequential

Batches are analyzed for parallel safety:

| Category | Tools |
|----------|-------|
| Never parallel | `clarify` |
| Parallel-safe (read-only) | `read_file`, `grep`, `find`, `ls` |
| Path-overlap detection | `write_file`, `patch` -- parallel if paths don't overlap |

Execution:
- **Concurrent path**: `ThreadPoolExecutor` with max 8 workers. Results collected in original order.
- **Sequential path**: Used for interactive tools or when parallelization is unsafe.

### Agent-Level Tool Interception

Certain tools are intercepted in `_invoke_tool()` before registry dispatch because they need agent state:
- `todo` -- Task management
- `memory` -- Memory operations
- `session_search` -- Session history search
- `delegate_task` -- Subagent delegation
- `clarify` -- User clarification request

### Checkpointing

Before destructive operations, the `CheckpointManager` snapshots the working directory:
- Triggered by: `write_file`, `patch`, destructive terminal commands
- Snapshot includes: working directory state
- Restore: user can revert to the checkpoint if needed

## Async Bridging

Python's async model requires bridging between sync and async handlers:
- Sync handlers are executed directly
- Async handlers are bridged using persistent event loops
- The registry's `dispatch()` handles both transparently

## Key Patterns for foundation_ai

1. **Self-registering tools** at import time -- just add a file and it's discovered
2. **Toolset system** with recursive inclusion and cycle detection
3. **Argument coercion** from strings to typed values using JSON Schema
4. **Parallel safety analysis** -- never parallel on `clarify`, path-overlap detection for file tools
5. **Agent-level interception** for tools that need agent state (todo, memory, delegate)
6. **Checkpointing** before destructive operations with restore capability
7. **ThreadPoolExecutor with 8 workers** for parallel tool execution
8. **Async bridging** via persistent event loops for mixed sync/async handlers
