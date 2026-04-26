# Hermes Agent -- Documentation Index

**Hermes** is a self-improving AI agent by Nous Research. Runs on any infrastructure, supports any LLM, communicates across 10+ messaging platforms, and has a closed learning loop with skills, memory, and context compression.

## Documents

### Foundation

| Document | What It Covers |
|----------|---------------|
| [00-overview.md](./00-overview.md) | What Hermes is, capabilities, philosophy |
| [01-architecture.md](./01-architecture.md) | Module graph, layers, communication patterns |

### Core Systems

| Document | What It Covers |
|----------|---------------|
| [02-agent-core.md](./02-agent-core.md) | AIAgent orchestrator, message loop, LLM adapters |
| [03-tool-system.md](./03-tool-system.md) | Tool registry, 40+ tools, dispatch, schemas |
| [04-gateway.md](./04-gateway.md) | Multi-platform messaging gateway |
| [05-memory-system.md](./05-memory-system.md) | Memory manager, providers, session search |
| [06-context-engine.md](./06-context-engine.md) | Context compression, DAG, summarization |

### Application Layer

| Document | What It Covers |
|----------|---------------|
| [07-cli-tui.md](./07-cli-tui.md) | CLI entry points, TUI, commands, configuration |
| [08-cron.md](./08-cron.md) | Scheduling, jobs, automation |
| [09-plugins.md](./09-plugins.md) | Plugin architecture and extension points |
| [10-platform-adapters.md](./10-platform-adapters.md) | Per-platform messaging adapter details |
| [11-data-flow.md](./11-data-flow.md) | End-to-end message flows with sequence diagrams |

## Quick Orientation

```
CLI/TUI ──→ AIAgent ──→ LLM Adapter ──→ Provider API
                │
                ├──→ Tool Registry ──→ 40+ Tools
                ├──→ Memory Manager ──→ Providers (Honcho, Mem0, etc.)
                ├──→ Context Engine ──→ Compression/Summarization
                └──→ Skills ──→ Self-created procedures

Gateway ──→ Platform Adapters ──→ Telegram, Discord, Slack, WhatsApp, ...
                │
                └──→ AIAgent (per-session)
```

## Source

`/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.Hermes/hermes-agent/`
