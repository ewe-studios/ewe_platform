# LangChain -- Blog Synthesis

Key principles, patterns, and aha moments from the LangChain blog.

## Documents

### Foundation
- [00 Core Principles & Architecture](00-overview.md) — LangChain ecosystem, six production agent features, harness-memory coupling, design principles
- [01 LangGraph Design](01-langgraph-design.md) — Pregel/BSP execution, checkpointing, streaming, human-in-the-loop, SDK evolution
- [05 LangGraph Source Architecture](05-langgraph-source.md) — Source code deep dive: Pregel class, PregelLoop.tick(), channels as state machine, versions_seen scheduler, Command/Send control flow, Checkpoint structure, JsonPlusSerializer

### Source-Grounded

### Harness & Patterns
- [02 Agent Harness Anatomy](02-agent-harness.md) — Harness lifecycle, middleware, skills, subagents, context compression, memory
- [03 Observability & Evaluation](03-observability-evaluation.md) — Runs/traces/threads, evaluation levels, production-as-teacher, improvement loop
- [04 Production Deep Agents](04-production-agents.md) — Runtime architecture, deployment, context management, authorization, security

## Core Principles

1. **LLMs are slow, flaky, and open-ended.** Every design decision follows from this.
2. **The biggest competitor to any framework is no framework.** APIs should feel like writing code.
3. **Memory isn't a plugin — it's the harness.** Context management is the harness's core responsibility.
4. **Production is the primary teacher.** Traces reveal failure modes you can't engineer.
5. **Observability and evaluation are inseparable.** You evaluate what you observe.
6. **Agent engineering is iterative.** Build → run → trace → evaluate → learn → improve → repeat.
7. **Low-level beats high-level for production.** Abstractions age poorly; building blocks endure.

## Quick Architecture

```
User → LangSmith (traces, evals) → Deep Agents (harness) → LangGraph (runtime) → Models
```

Six features every production agent needs: parallelization, streaming, task queue, checkpointing, human-in-the-loop, tracing.
