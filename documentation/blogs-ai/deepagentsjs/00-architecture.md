---
title: Deep Agents JS -- TypeScript Source Architecture
---

# Deep Agents JS -- TypeScript Source Architecture

## Purpose

Deep Agents JS is the TypeScript/JavaScript port of the Deep Agents harness. Same middleware architecture, same backend protocol pattern — adapted for the JS ecosystem.

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/deepagentsjs/`

## Monorepo Structure

```
deepagentsjs/
├── libs/
│   ├── deepagents/           # Core SDK — createDeepAgent(), middleware, backends
│   └── cli/                  # CLI tooling
├── evals/                    # Evaluation suite
├── internal/                 # Internal utilities
└── examples/                 # Usage examples
```

## Architecture

Mirrors the Python Deep Agents design:
- **`createDeepAgent()`** — main factory function returning a compiled LangGraph
- **Middleware stack** — intercepts LLM calls for dynamic tool filtering, prompt injection, context management
- **Backend protocol** — pluggable file storage with path-prefix routing
- **Sub-agent delegation** — isolated context windows via `task` tool
- **Summarization** — auto-compaction when context fills

Key JS-specific differences:
- Async/await throughout (no sync variant)
- Web Streams for streaming output
- AbortSignal for cancellation
- npm packages instead of pip

[Back to Deep Agents architecture → ../deepagents/00-architecture.md](../deepagents/00-architecture.md)
