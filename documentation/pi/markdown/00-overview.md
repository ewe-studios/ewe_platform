# Pi -- Overview

## What Pi Is

Pi is an open-source AI agent framework built as a TypeScript monorepo. It contains 7 npm packages that handle everything from raw LLM API calls to full interactive terminal agents, Slack bots, and web chat UIs.

The core philosophy: **minimal by default, extensible by design**. Each package does one thing. You compose them for your use case. You don't have to use the whole framework to get value from one piece.

## The 7 Packages

```
┌─────────────────────────────────────────────────────────────┐
│                        APPLICATIONS                          │
│                                                              │
│  pi-coding-agent    pi-mom         pi-pods      pi-web-ui   │
│  Interactive CLI     Slack bot      GPU mgmt     Browser UI  │
│  (flagship app)     (autonomous)   (vLLM)       (web chat)  │
│                                                              │
├─────────────────────────────────────────────────────────────┤
│                         RUNTIME                              │
│                                                              │
│  pi-agent-core              pi-tui                           │
│  Agent loop, tools,         Terminal UI framework,           │
│  events, state              components, rendering            │
│                                                              │
├─────────────────────────────────────────────────────────────┤
│                        FOUNDATION                            │
│                                                              │
│  pi-ai                                                       │
│  Unified LLM API: 20+ providers, streaming, tools,          │
│  thinking, context, OAuth                                    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### pi-ai (Foundation)

Unified API across 20+ LLM providers (OpenAI, Anthropic, Google, Mistral, xAI, Groq, Azure, Bedrock, etc.). Handles streaming, completions, tool calling, thinking/reasoning, token tracking, and cost estimation. Every other package depends on this.

### pi-agent-core (Runtime)

Stateful `Agent` class that manages conversation state, executes tools automatically, and emits events. Provides both a high-level `Agent` API and a low-level `agentLoop()` for direct control. Handles sequential and parallel tool execution, thinking budgets, and steering queues.

### pi-tui (Runtime)

Terminal UI framework with differential rendering. Components include `Text`, `Input`, `Editor`, `Markdown`, `SelectList`, `Image`, `Loader`. Supports overlays, keybindings, IME input, and terminal image protocols (Kitty, iTerm2).

### pi-coding-agent (Application)

The flagship: an interactive terminal coding agent. Four core tools (`read`, `write`, `edit`, `bash`), session management with branching and compaction, multiple run modes (interactive, print, JSON, RPC, SDK), extensibility via plugins/skills/themes.

### pi-mom (Application)

A Slack bot that runs as an autonomous coding agent. Manages its own workspace, creates its own skills, runs in Docker sandbox for security, handles file attachments, and maintains per-channel conversation history.

### pi-pods (Application)

CLI for managing GPU pods running vLLM. Setup, deploy models, manage multi-GPU tensor parallelism, test via interactive agent. Supports DataCrunch, RunPod, Vast.ai, AWS EC2.

### pi-web-ui (Application)

Web components (`ChatPanel`, `AgentInterface`, `ArtifactsPanel`) for building browser-based AI chat interfaces. Built with mini-lit web components and Tailwind CSS v4. IndexedDB-backed persistence.

## Design Principles

### 1. Packages Are Independent

`pi-ai` can be used without any other package. You can call `stream()` to get LLM responses without needing the agent, the TUI, or any application. Each package publishes to npm independently.

### 2. Composition Over Inheritance

Packages compose through function calls and event subscriptions, not class hierarchies. `pi-coding-agent` uses `pi-agent-core` by creating an `Agent` instance and subscribing to its events. It uses `pi-tui` by creating a `TUI` instance and wiring components.

### 3. Events Drive Everything

The `Agent` class emits 20+ event types: `agent_start`, `turn_start`, `message_update`, `tool_execution_start`, `tool_execution_end`, `turn_end`, `agent_end`. UI layers subscribe to these events and render incrementally during streaming.

### 4. Tools Are TypeBox Schemas

Every tool defines its parameters as a TypeBox schema. This gives you runtime validation, TypeScript type inference, and JSON Schema generation for LLM function calling -- from a single definition.

### 5. Multiple Run Modes

The coding agent supports interactive TUI, print (for piping), JSON (for scripting), RPC (for editors), and SDK (for embedding). Same agent core, different I/O surfaces.

## Technology Stack

| Concern | Choice |
|---------|--------|
| Language | TypeScript (strict) |
| Package management | npm workspaces |
| Build | tsgo |
| Test | Vitest |
| Lint | Biome |
| Web components | mini-lit |
| CSS (web-ui) | Tailwind v4 |
| Terminal rendering | ANSI escape codes, CSI 2026 sync |
| Storage (node) | File-based JSONL |
| Storage (browser) | IndexedDB |

## Monorepo Structure

```
pi-mono/
├── packages/
│   ├── ai/              → @mariozechner/pi-ai
│   ├── agent/           → @mariozechner/pi-agent-core
│   ├── coding-agent/    → @mariozechner/pi-coding-agent
│   ├── tui/             → @mariozechner/pi-tui
│   ├── mom/             → @mariozechner/pi-mom
│   ├── pods/            → @mariozechner/pi-pods
│   └── web-ui/          → @mariozechner/pi-web-ui
├── scripts/              Build, release, utility scripts
├── .pi/                  Configuration and prompt templates
├── package.json          Workspace root
├── tsconfig.json         Shared TypeScript config
└── biome.json            Linting config
```
