# Pi -- Architecture

## Package Dependency Graph

```mermaid
flowchart TD
    AI[pi-ai<br/>Unified LLM API]
    AGENT[pi-agent-core<br/>Agent Runtime]
    TUI[pi-tui<br/>Terminal UI]
    CODING[pi-coding-agent<br/>Interactive CLI]
    MOM[pi-mom<br/>Slack Bot]
    PODS[pi-pods<br/>GPU Management]
    WEB[pi-web-ui<br/>Browser Chat]

    AI --> AGENT
    AGENT --> CODING
    TUI --> CODING
    AGENT --> MOM
    CODING -.->|uses core| MOM
    AI --> WEB
    AGENT --> WEB
    TUI -.->|shared types| WEB
    AGENT --> PODS
```

### Dependency Direction

Dependencies flow **upward** -- applications depend on runtime, runtime depends on foundation. Nothing flows backward.

| Package | Depends On | Depended On By |
|---------|-----------|----------------|
| pi-ai | (none) | agent-core, web-ui, pods |
| pi-agent-core | pi-ai | coding-agent, mom, pods, web-ui |
| pi-tui | (none, standalone) | coding-agent, web-ui (types) |
| pi-coding-agent | pi-ai, pi-agent-core, pi-tui | mom (uses core internals) |
| pi-mom | pi-agent-core, pi-coding-agent | (none) |
| pi-pods | pi-agent-core | (none) |
| pi-web-ui | pi-ai, pi-agent-core | (none) |

## Communication Patterns

### 1. LLM API Calls (pi-ai)

```mermaid
sequenceDiagram
    participant App as Application
    participant AI as pi-ai
    participant Provider as LLM Provider

    App->>AI: stream(model, messages, tools)
    AI->>AI: Resolve provider from model ID
    AI->>Provider: HTTP request (provider-specific format)
    Provider-->>AI: SSE stream
    loop Each chunk
        AI-->>App: yield StreamEvent
    end
    AI-->>App: Final response with usage stats
```

pi-ai normalizes all provider differences. The application sees a single `StreamEvent` type regardless of whether the backend is OpenAI, Anthropic, or Gemini.

### 2. Agent Loop (pi-agent-core)

```mermaid
flowchart TD
    START[agent_start] --> TURN[turn_start]
    TURN --> CALL[Call LLM via pi-ai]
    CALL --> CHECK{Tool calls?}
    CHECK -->|Yes| EXEC[Execute tools]
    EXEC --> RESULT[Append tool results]
    RESULT --> CALL
    CHECK -->|No| STEERING{Steering queue?}
    STEERING -->|Yes| INJECT[Inject steering message]
    INJECT --> CALL
    STEERING -->|No| END_TURN[turn_end]
    END_TURN --> DONE{More work?}
    DONE -->|Yes| TURN
    DONE -->|No| END[agent_end]
```

The agent loop keeps calling the LLM until there are no more tool calls and no more steering messages. Each iteration is a "turn."

### 3. Event-Driven UI (pi-tui ← pi-agent-core)

```mermaid
sequenceDiagram
    participant User
    participant TUI as pi-tui
    participant Agent as pi-agent-core
    participant AI as pi-ai

    User->>TUI: Types message
    TUI->>Agent: agent.run(message)
    Agent->>AI: stream(model, messages)
    AI-->>Agent: StreamEvent (text chunk)
    Agent-->>TUI: message_update event
    TUI->>TUI: Differential render (only changed lines)
    AI-->>Agent: StreamEvent (tool call)
    Agent-->>TUI: tool_execution_start event
    Agent->>Agent: Execute tool
    Agent-->>TUI: tool_execution_end event
    TUI->>TUI: Render tool result
```

The TUI subscribes to agent events and renders incrementally. Only changed terminal lines are redrawn (differential rendering via CSI 2026 synchronized output).

### 4. Slack Bot (pi-mom)

```mermaid
flowchart TD
    SLACK[Slack Socket Mode] --> MSG[Incoming Message]
    MSG --> SESSION[Load/Create Session]
    SESSION --> AGENT[Run Agent]
    AGENT --> TOOLS{Tool calls?}
    TOOLS -->|bash| SANDBOX[Docker Sandbox]
    TOOLS -->|file ops| WORKSPACE[Bot Workspace]
    TOOLS -->|skill| SKILL[Execute CLI Skill]
    SANDBOX --> RESULT[Tool Result]
    WORKSPACE --> RESULT
    SKILL --> RESULT
    RESULT --> AGENT
    AGENT --> REPLY[Post Reply to Slack]
```

Mom manages per-channel sessions with persistent history. All bash commands execute inside a Docker container for security isolation.

## Data Model

### Messages

All packages share a common message format:

```typescript
type Message = UserMessage | AssistantMessage | ToolResultMessage;

interface UserMessage {
  role: 'user';
  content: string | ContentPart[];
}

interface AssistantMessage {
  role: 'assistant';
  content: string | ContentPart[];
  tool_calls?: ToolCall[];
}

interface ToolResultMessage {
  role: 'tool';
  tool_call_id: string;
  content: string;
}
```

Messages flow through the system unchanged. pi-ai formats them for each provider. pi-agent-core manages the conversation array. Applications append user messages and read assistant responses.

### Tool Definitions

```typescript
interface AgentTool<T extends TSchema = TSchema> {
  name: string;
  description: string;
  parameters: T;       // TypeBox schema
  execute: (
    id: string,
    params: Static<T>,
    signal: AbortSignal,
    onUpdate?: (update: string) => void
  ) => Promise<ToolResult>;
}
```

Tools are defined once with a TypeBox schema. The schema serves three purposes:
1. TypeScript type inference (compile-time safety)
2. Runtime input validation (before execution)
3. JSON Schema generation (sent to LLM for function calling)

### Events

```typescript
type AgentEvent =
  | { type: 'agent_start' }
  | { type: 'turn_start'; turn: number }
  | { type: 'message_update'; content: string; delta: string }
  | { type: 'tool_execution_start'; tool: string; id: string; params: unknown }
  | { type: 'tool_execution_end'; tool: string; id: string; result: ToolResult }
  | { type: 'thinking_update'; content: string }
  | { type: 'turn_end'; turn: number }
  | { type: 'agent_end' }
  // ... 12+ more event types
```

Events are the decoupling mechanism. The agent runtime doesn't know about the TUI. The TUI doesn't know about Slack. They communicate through events.

## Build System

```
npm workspaces (monorepo)
  ↓
tsgo (TypeScript compilation per package)
  ↓
Vitest (test runner)
  ↓
Biome (linting + formatting)
```

Each package builds independently. Workspace dependencies are resolved by npm. The build order follows the dependency graph: pi-ai first, then pi-agent-core, then applications.

### Package Exports

Each package uses Node.js subpath exports for tree-shaking:

```json
// pi-ai package.json (simplified)
{
  "exports": {
    ".": "./dist/index.js",
    "./anthropic": "./dist/providers/anthropic.js",
    "./openai": "./dist/providers/openai.js",
    "./openai-responses": "./dist/providers/openai-responses.js",
    "./bedrock-provider": "./dist/providers/bedrock.js"
  }
}
```

Applications import only the providers they need. Unused providers are not bundled.

## Key Directories

```
packages/ai/src/
  ├── index.ts              Public API (getModel, stream, complete)
  ├── types.ts              Core types (Provider, Api, Context, Model)
  ├── providers/            Per-provider implementations
  │   ├── anthropic.ts
  │   ├── openai.ts
  │   ├── openai-responses.ts
  │   ├── google.ts
  │   ├── bedrock.ts
  │   └── ... (15+ more)
  ├── tools/                Tool calling utilities
  └── context/              Context serialization

packages/agent/src/
  ├── agent.ts              Agent class (high-level API)
  ├── agent-loop.ts         Core loop implementation
  ├── events.ts             Event type definitions
  ├── tools.ts              Tool execution and validation
  └── messages.ts           Message transformation

packages/coding-agent/src/
  ├── cli.ts                CLI entry point
  ├── core/
  │   ├── agent-session.ts  Session management
  │   ├── tools/            Built-in tools (read, write, edit, bash)
  │   ├── extensions/       Plugin system
  │   └── compaction/       Context window management
  └── modes/                Run modes (interactive, print, json, rpc)

packages/tui/src/
  ├── tui.ts                Main TUI class
  ├── terminal.ts           Terminal abstraction
  ├── components/           Built-in components
  │   ├── text.ts
  │   ├── input.ts
  │   ├── editor.ts
  │   ├── markdown.ts
  │   ├── select-list.ts
  │   └── image.ts
  └── rendering/            Differential rendering engine
```
