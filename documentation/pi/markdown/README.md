# Pi -- Documentation Index

**Pi** is a modular AI agent framework. 7 TypeScript packages that work independently or compose together into interactive coding agents, Slack bots, GPU pod managers, and web UIs.

## Documents

### Foundation

| Document | What It Covers |
|----------|---------------|
| [00-overview.md](./00-overview.md) | What Pi is, philosophy, the 7 packages at a glance |
| [01-architecture.md](./01-architecture.md) | Package dependency graph, communication patterns, build system |

### Package Deep Dives

| Document | Package | Purpose |
|----------|---------|---------|
| [02-ai-package.md](./02-ai-package.md) | `pi-ai` | Unified LLM API across 20+ providers |
| [03-agent-core.md](./03-agent-core.md) | `pi-agent-core` | Stateful agent with tool execution and events |
| [04-coding-agent.md](./04-coding-agent.md) | `pi-coding-agent` | Interactive terminal coding agent (flagship) |
| [05-tui.md](./05-tui.md) | `pi-tui` | Terminal UI framework with differential rendering |
| [06-mom.md](./06-mom.md) | `pi-mom` | Slack bot with Docker sandbox and skills |
| [07-pods.md](./07-pods.md) | `pi-pods` | GPU pod management for vLLM deployment |
| [08-web-ui.md](./08-web-ui.md) | `pi-web-ui` | Web components for AI chat interfaces |

### Cross-Cutting Concerns

| Document | What It Covers |
|----------|---------------|
| [09-tool-system.md](./09-tool-system.md) | Tool definition, TypeBox schemas, execution lifecycle |
| [10-extension-system.md](./10-extension-system.md) | Extensions, skills, prompt templates, themes |
| [11-data-flow.md](./11-data-flow.md) | End-to-end request flows with sequence diagrams |
| [12-sessions.md](./12-sessions.md) | Pi JSONL tree + Hermes SQLite + comparison |
| [13-agent-loop.md](./13-agent-loop.md) | Agent loop deep dive: loop mechanics, multi-turn, session, message, tool handling |

### Model Interop & Memory Expansion

| Document | What It Covers |
|----------|---------------|
| [14-model-providers.md](./14-model-providers.md) | 20+ providers, API registry, lazy loading, OpenAI-compatible endpoints, streaming |
| [15-memory-deep.md](./15-memory-deep.md) | Message-based memory, context window management, compaction, session persistence |
| [16-multi-model.md](./16-multi-model.md) | Model switching, parallel tool execution, background runs, extension-driven routing |

### Deep Dives

| Document | What It Covers |
|----------|---------------|
| [17-context-compression.md](./17-context-compression.md) | Token-budget compaction, findCutPoint backward walk, split turn parallel summarization, FileOperations tracking, extension hook |

## Quick Orientation

```
pi-ai              ← LLM abstraction (foundation)
  ↓
pi-agent-core      ← Agent loop + tool execution
  ↓
pi-coding-agent    ← Interactive CLI (uses pi-tui for UI)
  ↓
pi-mom             ← Slack bot (uses coding-agent core)
pi-web-ui          ← Browser chat (uses ai + agent)
pi-pods            ← GPU management (standalone + agent)
```

## Source

`/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.Pi/pi-mono/`
