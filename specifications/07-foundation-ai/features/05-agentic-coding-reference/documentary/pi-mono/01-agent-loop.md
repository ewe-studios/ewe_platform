# pi-mono Agent Loop

## File Locations

- Core loop: `packages/agent/src/agent-loop.ts` (lines 155-232)
- Agent class: `packages/agent/src/agent.ts` (lines 158-543)
- Agent types: `packages/agent/src/types.ts`

## Nested Loop Structure

The agent loop has a two-level nested structure:

```
Outer Loop (follow-up messages)
  └── Inner Loop (tool calls + steering)
        ├── streamAssistantResponse()
        ├── executeToolCalls()
        ├── pollSteeringQueue()
        └── check follow-up queue
```

### Outer Loop -- Follow-up Messages

Handles follow-up messages that extend the conversation after the agent would naturally stop. This enables patterns like:
- User sends a follow-up after the assistant finishes
- System-generated follow-up prompts (e.g., "do you want me to continue?")
- Multi-turn conversations that don't require a new `prompt()` call

### Inner Loop -- Tool Calls and Steering

Processes each LLM turn:

1. **Inject pending steering messages** -- User interruptions queued via `agent.steer()` are injected into the context before the next API call
2. **Call `streamAssistantResponse()`** -- Transforms `AgentMessage[]` to LLM `Message[]`, calls the provider, streams back an `AssistantMessage`
3. **Check stopReason** -- If "error" or "aborted", exit the loop immediately
4. **Execute tool calls** -- If tool calls exist, execute them via `executeToolCalls()` (parallel or sequential based on config)
5. **Push tool results** -- Tool result messages are added to the context for the next turn
6. **Poll steering queue** -- Check for new steering messages while tools execute
7. **Check follow-up** -- If no more tool calls or steering, check the follow-up queue

## Agent Class (Stateful Wrapper)

The `Agent` class wraps the low-level loop with:

### Mutable State
- `systemPrompt` -- System prompt string
- `model` -- Current `Model<TApi>` descriptor
- `thinkingLevel` -- Current reasoning/thinking level
- `tools` -- Active `AgentTool[]`
- `messages` -- Conversation history (`AgentMessage[]`)
- `isStreaming` -- Whether currently streaming

### Lifecycle Control
- `AbortController` -- Cancellation support for in-flight requests
- `prompt()` -- Entry point for new user messages
- `continue()` -- Resume with an empty prompt (continue the conversation)
- `steer()` -- Inject user interruptions mid-run
- `followUp()` -- Queue follow-up messages for after the current turn

### Event Processing
- `processEvents()` -- Reduces state per event type, then awaits all subscribers
- Event types: `agent_start`, `agent_end`, `turn_start`, `turn_end`, `message_delta`, `message_done`, `tool_execution_start`, `tool_execution_end`, `tool_execution_stream`

## Event Protocol

```typescript
type AgentEvent =
  | { type: "agent_start"; messages: AgentMessage[] }
  | { type: "agent_end"; messages: AgentMessage[] }
  | { type: "turn_start" }
  | { type: "turn_end"; usage: Usage }
  | { type: "message_start"; message: AssistantMessage }
  | { type: "message_delta"; delta: string }
  | { type: "message_done"; message: AssistantMessage }
  | { type: "tool_execution_start"; toolCall: ToolCall }
  | { type: "tool_execution_end"; result: ToolResultMessage }
  | { type: "tool_execution_stream"; toolCallId: string; delta: string };
```

Every event flows through `processEvents()` which:
1. Updates local state (messages, usage, etc.)
2. Notifies all subscribers
3. Returns a promise that resolves when all subscribers are done

## Streaming Response Flow

`streamAssistantResponse()` is the core function that:
1. Transforms `AgentMessage[]` (application format) to `Message[]` (LLLM format)
2. Applies provider-specific message transformation via `transformMessages()`
3. Calls the provider's `stream()` or `streamSimple()` function
4. Consumes the `AssistantMessageEvent` stream, accumulating content blocks
5. Emits `message_delta` events for text, `toolcall_start/delta/end` events for tools
6. Returns the final `AssistantMessage` with all content blocks and usage

## Tool Execution Flow (`executeToolCalls()`)

```
executeToolCalls(toolCalls, config, context)
  ├── determine execution mode (sequential if any tool requires it)
  ├── for each tool call:
  │   ├── prepareToolCall() -- find definition, validate args, beforeToolCall hook
  │   ├── if blocked, return error result
  │   ├── executePreparedToolCall() -- call tool.execute(), stream partial updates
  │   ├── finalizeExecutedToolCall() -- afterToolCall hook
  │   └── emit tool_execution_end event + create ToolResultMessage
  └── return all tool result messages
```

### Execution Mode
- **Sequential**: Default. Tools execute one at a time in order
- **Parallel**: Configurable. All tools start simultaneously
- **Mixed**: Sequential if ANY tool in the batch has `executionMode: "sequential"`, otherwise parallel

## Steering and Interruption

The `steer()` method allows mid-run user interruptions:
1. Messages are queued in a steering queue
2. The next turn checks the steering queue before calling the API
3. Pending steering messages are injected into the context
4. The LLM sees the interruption as part of the conversation history

This enables real-time user interruption of long-running tool execution chains without cancelling the entire run.

## Key Patterns for foundation_ai

1. **Nested loop structure** cleanly separates tool-call continuation from conversation continuation
2. **Event protocol** enables multiple UI backends from the same core loop
3. **Steering queue** provides interruption without cancellation
4. **beforeToolCall/afterToolCall hooks** enable extension points
5. **AbortController** for clean cancellation of in-flight requests
6. **State reduction per event** keeps the agent's view of the conversation consistent
