# Parallel Tool Execution Behavior

## pi-mono Approach

### Execution Mode Determination

- **Sequential**: Default. Tools execute one at a time in order
- **Parallel**: Configurable. All tools start simultaneously
- **Mixed**: Sequential if ANY tool in the batch has `executionMode: "sequential"`, otherwise parallel

### Tool Author Responsibility

Tool authors mark tools as sequential or parallel:

| Mode | Examples |
|------|----------|
| Parallel | `read`, `grep`, `find`, `ls` (read-only) |
| Sequential | `bash`, `write`, `edit` (side effects) |

If any tool in a batch requires sequential execution, ALL tools in that batch run sequentially.

### No Path Overlap Detection

pi-mono does not analyze whether parallel file operations target the same paths. It relies on tool authors to mark tools correctly.

## hermes-agent Approach

### Parallel Safety Analysis

Batches are analyzed for parallel safety:

| Category | Tools |
|----------|-------|
| Never parallel | `clarify` |
| Parallel-safe (read-only) | `read_file`, `grep`, `find`, `ls` |
| Path-overlap detection | `write_file`, `patch` -- parallel if paths don't overlap |

### Path Overlap Detection

For file-writing tools, hermes-agent checks if the target paths overlap:
- If two tools target the same file or overlapping paths, they run sequentially
- If paths are disjoint, they run in parallel

### ThreadPoolExecutor

- **Max workers**: 8
- **Results collected in original order** -- order is preserved regardless of completion time
- **Sequential path**: Used for interactive tools or when parallelization is unsafe

### _NEVER_PARALLEL_TOOLS

```python
_NEVER_PARALLEL_TOOLS = {"clarify"}
```

The `clarify` tool (user interaction) can never run in parallel.

## Comparison

| Aspect | pi-mono | hermes-agent |
|--------|---------|-------------|
| Default | Sequential | Analyzed |
| Parallelism control | Tool author flag | Safety analysis + path overlap |
| Path overlap | Not detected | Detected for file tools |
| Max parallel | Unlimited | 8 workers |
| Never-parallel | None | `clarify` |
| Result ordering | Preserved | Preserved |

## Key Patterns for foundation_ai

1. **Tool author flag** (pi-mono) is simpler but requires discipline
2. **Path overlap detection** (hermes-agent) is safer but more complex
3. **Max workers limit** (8 in hermes-agent) prevents resource exhaustion
4. **Never-parallel list** for tools that must always run alone
5. **Result ordering preserved** regardless of execution order
6. **Hybrid approach possible** -- flag for coarse control, path overlap for fine control
