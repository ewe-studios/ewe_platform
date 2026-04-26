# Pi -- Data Flow (End-to-End)

## Overview

This document traces data through the full Pi stack for three common scenarios:
1. Interactive chat with tool execution
2. Slack bot handling a message
3. Session compaction

## Flow 1: Interactive Chat with Tool Execution

A user types a message in the terminal. The agent reads a file, edits it, and responds.

```mermaid
sequenceDiagram
    participant User
    participant TUI as pi-tui<br/>Terminal UI
    participant Session as AgentSession<br/>pi-coding-agent
    participant Agent as Agent<br/>pi-agent-core
    participant AI as pi-ai
    participant Claude as Anthropic API

    User->>TUI: Types "fix the typo in config.ts"
    TUI->>TUI: Editor captures input
    TUI->>Session: session.run("fix the typo in config.ts")
    Session->>Session: Add user message to history
    Session->>Session: Build system prompt + context files
    Session->>Agent: agent.run(messages, tools)

    Agent->>Agent: Emit agent_start
    Agent->>Agent: Emit turn_start (turn 1)
    Agent->>AI: stream(model, messages, tools)
    AI->>AI: Format for Anthropic (messages + tool schemas)
    AI->>Claude: POST /v1/messages (SSE stream)

    Claude-->>AI: text delta: "I'll read config.ts"
    AI-->>Agent: StreamEvent(text_delta)
    Agent-->>TUI: message_update event
    TUI->>TUI: Differential render (append text)

    Claude-->>AI: tool_call: read({ path: "config.ts" })
    AI-->>Agent: StreamEvent(tool_call)
    Agent->>Agent: Validate read params (TypeBox)
    Agent->>Agent: Emit tool_execution_start
    Agent-->>TUI: tool_execution_start event
    TUI->>TUI: Show "Reading config.ts..."

    Agent->>Session: Execute read tool
    Session-->>Agent: File contents
    Agent->>Agent: Emit tool_execution_end
    Agent-->>TUI: tool_execution_end event
    TUI->>TUI: Show file contents (collapsed)

    Agent->>Agent: Append tool result to messages
    Agent->>Agent: Emit turn_end (turn 1)
    Agent->>Agent: Emit turn_start (turn 2)
    Agent->>AI: stream(model, messages + tool result, tools)
    AI->>Claude: POST /v1/messages

    Claude-->>AI: text: "I found the typo. Fixing..."
    Claude-->>AI: tool_call: edit({ path: "config.ts", old: "teh", new: "the" })
    AI-->>Agent: StreamEvent(tool_call)
    Agent->>Session: Execute edit tool
    Session-->>Agent: File edited
    Agent->>AI: Tool result → Claude

    Claude-->>AI: text: "Fixed the typo on line 42."
    AI-->>Agent: StreamEvent(text_delta)
    Agent-->>TUI: message_update event
    TUI->>TUI: Render final response

    Agent->>Agent: No more tool calls
    Agent->>Agent: Emit turn_end, agent_end
    Session->>Session: Save history to disk
```

### What Happens at Each Layer

| Layer | Input | Output | Responsibility |
|-------|-------|--------|---------------|
| TUI | Keystrokes | Rendered terminal output | Capture input, render output, show progress |
| AgentSession | User message string | Completed conversation | Manage history, context files, compaction |
| Agent | Messages + tools | Final response + events | Loop until done, validate tools, emit events |
| pi-ai | Normalized request | Streaming events | Provider-specific formatting, HTTP, SSE parsing |
| Anthropic API | HTTP request | SSE stream | LLM inference |

## Flow 2: Slack Bot Message Handling

A user messages @mom in a Slack channel.

```mermaid
sequenceDiagram
    participant Slack as Slack
    participant Socket as Socket Mode
    participant Mom as pi-mom
    participant Session as Channel Session
    participant Agent as pi-agent-core
    participant AI as pi-ai
    participant Sandbox as Docker

    Slack->>Socket: WebSocket event: message in #general
    Socket->>Mom: Message: "@mom run the tests"
    Mom->>Mom: Extract channel ID, user, text
    Mom->>Session: Load session for #general

    Session->>Session: Read log.jsonl (history)
    Session->>Session: Read MEMORY.md (channel + global)
    Session->>Session: Build system prompt with memory

    Mom->>Agent: agent.run(messages, tools)
    Agent->>AI: stream(model, messages, tools)
    AI-->>Agent: tool_call: bash("npm test")

    Agent->>Sandbox: Execute in Docker container
    Sandbox->>Sandbox: docker exec -i sandbox npm test
    Sandbox-->>Agent: Test output (stdout + stderr)

    Agent->>AI: Tool result → LLM
    AI-->>Agent: text: "Tests passed! 42/42 specs green."

    Agent-->>Mom: agent_end event
    Mom->>Session: Save log.jsonl
    Mom->>Session: Sync MEMORY.md if updated
    Mom->>Slack: Post reply in thread
```

### Key Differences from Interactive Flow

1. **No TUI** -- Mom posts to Slack instead of rendering to a terminal
2. **Docker sandbox** -- Bash commands execute in an isolated container
3. **Per-channel sessions** -- Each Slack channel has its own conversation state
4. **Working memory** -- MEMORY.md is read at session start and can be updated by the agent
5. **Persistent workspace** -- Files created by the agent persist across conversations

## Flow 3: Session Compaction

When the conversation exceeds the model's context window.

```mermaid
sequenceDiagram
    participant Session as AgentSession
    participant Compact as Compaction Engine
    participant AI as pi-ai
    participant LLM as LLM

    Session->>Session: Check token count
    Note over Session: 180K tokens, limit is 200K

    Session->>Compact: Compact history
    Compact->>Compact: Split: recent (last 20 msgs) + old (80 msgs)
    Compact->>Compact: Format old messages for summarization

    Compact->>AI: complete(model, "Summarize this conversation...")
    AI->>LLM: POST /v1/messages
    LLM-->>AI: Summary text (2K tokens)
    AI-->>Compact: Summary

    Compact->>Session: Replace old messages with summary
    Note over Session: 1 summary msg + 20 recent = ~25K tokens

    Session->>Session: Continue conversation with compacted history
```

### Compaction Strategy

```
Before compaction (180K tokens):
  [system_prompt] [msg_1] [msg_2] ... [msg_80] [msg_81] ... [msg_100]
                  |___ old (to summarize) ___|  |___ recent (keep) ___|

After compaction (~25K tokens):
  [system_prompt] [summary_of_1_to_80] [msg_81] ... [msg_100]
```

The summary includes:
- Key decisions made
- Files that were modified
- Errors encountered and how they were resolved
- The current state of the task

Recent messages are kept verbatim because they contain the most relevant context for the next LLM call.

## Data Formats

### Message Storage (JSONL)

Sessions are stored as newline-delimited JSON:

```jsonl
{"role":"user","content":"fix the bug in auth.ts","timestamp":"2026-04-26T10:00:00Z"}
{"role":"assistant","content":"I'll look at auth.ts...","tool_calls":[{"name":"read","arguments":{"path":"auth.ts"}}],"timestamp":"2026-04-26T10:00:01Z"}
{"role":"tool","tool_call_id":"tc_1","content":"// auth.ts contents...","timestamp":"2026-04-26T10:00:02Z"}
{"role":"assistant","content":"Found the issue. The token...","timestamp":"2026-04-26T10:00:05Z"}
```

### Context Serialization

pi-ai's context serialization format is provider-agnostic:

```json
{
  "format": "pi-context-v1",
  "model": "claude-sonnet-4-6",
  "messages": [...],
  "systemPrompt": "...",
  "tools": [...],
  "usage": { "input": 5000, "output": 2000 },
  "timestamp": "2026-04-26T10:00:00Z"
}
```

This can be loaded with a different model. pi-ai handles format conversion between providers.

### Event Stream

Events from the agent are emitted as typed objects:

```typescript
// During streaming, a consumer might receive:
{ type: 'agent_start' }
{ type: 'turn_start', turn: 1 }
{ type: 'message_update', content: 'I', delta: 'I' }
{ type: 'message_update', content: "I'll", delta: "'ll" }
{ type: 'message_update', content: "I'll read", delta: ' read' }
{ type: 'tool_call_start', tool: 'read', id: 'tc_1' }
{ type: 'tool_call_complete', tool: 'read', id: 'tc_1', params: { path: 'auth.ts' } }
{ type: 'tool_execution_start', tool: 'read', id: 'tc_1', params: { path: 'auth.ts' } }
{ type: 'tool_execution_end', tool: 'read', id: 'tc_1', result: { content: '...' } }
{ type: 'turn_end', turn: 1, usage: { input: 5000, output: 200 } }
{ type: 'turn_start', turn: 2 }
// ... more events ...
{ type: 'agent_end', result: { ... } }
```

Each event carries enough context to render the complete UI state. The TUI, Slack bot, and web UI all consume the same event stream with different rendering logic.
