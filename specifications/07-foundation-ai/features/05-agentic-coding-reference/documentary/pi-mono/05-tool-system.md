# pi-mono Tool System

## File Locations

- AI types: `packages/ai/src/types.ts` (Tool, ToolCall, ToolResultMessage)
- Agent types: `packages/agent/src/types.ts` (AgentTool, ToolExecutionMode)
- Built-in tools: `packages/coding-agent/src/core/tools/index.ts`
- Tool execution: `packages/agent/src/agent-loop.ts` (lines 336-663)

## Three-Layer Tool Definition

### Layer 1: Base Tool (pi-ai)

```typescript
interface Tool {
  name: string;
  description: string;
  parameters: TSchema;  // TypeBox schema
}
```

The base tool is just a name, description, and JSON Schema. It is provider-agnostic and used for formatting tool definitions to send to LLMs.

### Layer 2: AgentTool (pi-agent-core)

```typescript
interface AgentTool extends Tool {
  label: string;  // Human-readable label
  prepareArguments?(args: any, context): any;  // Compatibility shim
  execute(args: any, context, options): Promise<ToolResult>;  // Async execution
  executionMode: "sequential" | "parallel";  // Parallel safety
}
```

AgentTool adds execution capability. The `execute()` method performs the actual work, streaming partial updates via a callback.

### Layer 3: ToolDefinition (coding-agent extensions)

```typescript
interface ToolDefinition {
  name: string;
  description: string;
  parameters: TSchema;
  execute(...): Promise<ToolResult>;
  executionMode: "sequential" | "parallel";
  // Plus: rendering info, source info, metadata
}
```

Extension-level tools add rendering metadata, source information, and more. This is how extensions contribute tools to the agent.

## Built-in Tools

| Tool | Purpose | Execution Mode |
|------|---------|----------------|
| `read` | Read file contents with line range support | Parallel |
| `bash` | Execute shell commands with timeout, streaming output | Sequential |
| `edit` | Edit files with search/replace or line-based edits | Sequential |
| `write` | Write/overwrite file contents | Sequential |
| `grep` | Search file contents with regex | Parallel |
| `find` | Find files by pattern | Parallel |
| `ls` | List directory contents | Parallel |

## Tool Execution Flow

```
executeToolCalls(toolCalls, config, context)
  │
  ├── 1. Filter tool calls from assistant message content
  ├── 2. Determine execution mode:
  │      sequential if any tool.requiresSequential, else parallel
  ├── 3. Per tool call:
  │      a. prepareToolCall():
  │         - Find tool definition by name
  │         - Prepare arguments (compatibility shim)
  │         - Validate against JSON Schema
  │         - Run beforeToolCall hook
  │      b. If blocked (hook returns false), return error result
  │      c. executePreparedToolCall():
  │         - Call tool.execute(args, context, options)
  │         - Stream partial updates via callback
  │      d. finalizeExecutedToolCall():
  │         - Run afterToolCall hook for result overrides
  │      e. Emit tool_execution_end event
  │      f. Create ToolResultMessage and push to context
  └── 4. Return all tool result messages
```

## Tool Argument Validation

Tools use TypeBox schemas (`@sinclair/typebox`) for argument validation:

```typescript
validateToolArguments(tool, args): ValidationResult
```

This validates the LLM-returned arguments against the tool's `parameters` schema before execution. If validation fails, an error result is returned immediately without calling the tool's `execute()` method.

## Parallel vs Sequential Execution

The execution mode is determined by the batch:
- If ANY tool in the batch has `executionMode: "sequential"`, all tools execute sequentially
- Otherwise, all tools execute in parallel

This is a simpler approach than hermes-agent's path-overlap detection. pi-mono relies on tool authors to mark tools as sequential when needed (e.g., `bash`, `write`, `edit` are sequential; `read`, `grep`, `find`, `ls` are parallel).

## beforeToolCall / afterToolCall Hooks

These hooks enable extensions to intercept tool execution:

- **beforeToolCall(toolCall, context)** -- Called before execution. Can:
  - Modify arguments
  - Block execution (return false)
  - Return a cached result (short-circuit)

- **afterToolCall(toolCall, result, context)** -- Called after execution. Can:
  - Override the result
  - Transform the output
  - Record metrics

## Tool HTML Rendering

For session export (`packages/coding-agent/src/core/export-html/tool-renderer.ts`), tools render as formatted HTML with syntax highlighting. This is used for the `--export` feature that converts JSONL sessions to browsable HTML.

## Key Patterns for foundation_ai

1. **Three-layer tool definition** cleanly separates: schema (pi-ai) -> execution (pi-agent-core) -> extension metadata (coding-agent)
2. **TypeBox schema validation** ensures type safety before execution
3. **beforeToolCall/afterToolCall hooks** provide extension points for caching, modification, and blocking
4. **Sequential/parallel flag** per tool is simpler than automatic path-overlap analysis
5. **Streaming partial updates** via callback during tool execution enables real-time TUI feedback
6. **Tool result as ToolResultMessage** -- results flow back into the conversation context naturally
