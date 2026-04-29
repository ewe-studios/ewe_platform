# Mastra -- Documentation Index

**Mastra** is a TypeScript-first AI agent framework. pnpm workspace monorepo with `@mastra/core` at the center, a model router supporting 200+ providers, workflow-based agentic loop, built-in memory with semantic recall, processor pipeline, and background task execution.

## Documents

### Foundation

| Document | What It Covers |
|----------|---------------|
| [00-overview.md](./00-overview.md) | What Mastra is, philosophy, architecture |
| [01-architecture.md](./01-architecture.md) | Package map, dependency graph, layers |

### Core Systems

| Document | What It Covers |
|----------|---------------|
| [02-agent-core.md](./02-agent-core.md) | Agent class, generate/stream, execution |
| [03-agent-loop.md](./03-agent-loop.md) | Workflow-based agentic loop deep dive |
| [04-tool-system.md](./04-tool-system.md) | Tool calling, suspension, approval, background |
| [05-model-router.md](./05-model-router.md) | Provider resolution, OpenAI-compatible, gateway plugins |
| [06-memory-system.md](./06-memory-system.md) | Memory, semantic recall, working memory |
| [07-processors.md](./07-processors.md) | Input/output/error processor pipeline |
| [08-multi-model.md](./08-multi-model.md) | Model fallbacks, background tasks, delegation |

### Data Flow and Comparison

| Document | What It Covers |
|----------|---------------|
| [09-data-flow.md](./09-data-flow.md) | End-to-end flows with sequence diagrams |
| [10-comparison.md](./10-comparison.md) | Mastra vs Pi vs Hermes |

## Deep Dives

| Document | What It Covers |
|----------|---------------|
| [11-context-compression.md](./11-context-compression.md) | Memory-driven context selection vs post-hoc compression |
| [12-async-typescript.md](./12-async-typescript.md) | TransformStream, Promise.allSettled, pubsub, LLM recorder |
| [13-multi-model-deep.md](./13-multi-model-deep.md) | Provider resolution, fallback chains, error classification |
| [14-rl-training-traces.md](./14-rl-training-traces.md) | OTEL spans, LLM recorder, training trace comparison |

## Ecosystem

| Document | What It Covers |
|----------|---------------|
| [15-ecosystem.md](./15-ecosystem.md) | Production services, workshops, templates (14), skills infrastructure |
| [16-plugin-ecosystem.md](./16-plugin-ecosystem.md) | Auth (9), stores (22), voice (14), observability (14), deployers, adapters |
| [17-examples.md](./17-examples.md) | examples/agent and agent-v6: primitives, elicitation, presets, structured output |
| [18-docs.md](./18-docs.md) | Official Docusaurus docs site: 5 collections, 18 sections, 13 tutorials |

## Quick Orientation

```
Agent.generate()
  → #execute()
    → memory.query()          ← Load thread history, working memory
    → inputProcessors         ← Memory, Skills, Workspace
    → loop()
      → MODEL_STEP            ← Model router → LLM call
      → TOOL_STEP             ← Execute tools (concurrent or sequential)
      → MODEL_STEP            ← LLM with tool results
      → ...repeat...
    → outputProcessors        ← Structured output, formatting
    → errorProcessors         ← Recovery on failure
```

## Source

`/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.mastra-ai/mastra/`
