# Agent Observability -- Comprehensive Guide

How to observe, evaluate, and continuously improve AI agents.

## Documents

- [00 Agent Observability](00-overview.md) — Observability gap, runs/traces/threads, LangSmith stack, building from scratch, evaluation methods, production monitoring, improvement loop
- [01 LangSmith Internals](01-langsmith-internals.md) — Source code deep dive (v0.8.0): contextvars tracing, dotted_order encoding, UUIDv7, background thread batching, write replicas, provider wrappers, evaluation engine

## Core Principles

1. **Traces are the new source of truth.** Behavior emerges at runtime, not in code.
2. **Observability → Evaluation → Improvement.** The three are inseparable.
3. **Agent observability ≠ software observability.** You capture reasoning, not service calls.
4. **Production is the primary teacher.** Traces reveal failure modes you can't engineer.

## Quick Stack

```
Agent Code → Traces (runs, traces, threads) → Evaluation → Improvement
                  ↓
            Monitoring & Alerting
```

## Building From Scratch

Start with: trace capture decorator, trace viewing UI, error tracking, cost tracking, basic evals.
Add later: LLM-as-judge, dataset management, prompt versioning, thread analysis, alerting.
