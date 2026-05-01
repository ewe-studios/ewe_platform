# Blogs AI -- Industry Research Synthesis

Core ideas, principles, and aha moments from leading AI engineering blogs and source code.

## LangChain
[LangChain README → langchain/README.md](langchain/README.md)

Five documents covering LangChain's evolution from chains to the LangGraph/Deep Agents/LangSmith ecosystem:
- Core principles, six production agent features, harness-memory coupling
- LangGraph design: Pregel/BSP, checkpointing, streaming, human-in-the-loop
- Agent harness anatomy: middleware, skills, subagents, context compression
- Observability & evaluation: runs/traces/threads, eval levels, improvement loop
- Production deep agents: runtime, deployment, authorization, security
- [LangGraph source architecture](langchain/05-langgraph-source.md) — Source code deep dive: Pregel class, PregelLoop.tick(), channels as state machine, versions_seen scheduler, Command/Send control flow

## LangGraph.js
[LangGraph.js README → langgraphjs/README.md](langgraphjs/README.md)

TypeScript port of LangGraph — same Pregel algorithm, JS-native concurrency:
- [Source architecture](langgraphjs/00-source.md) — Pregel runtime, channels, checkpointing, StateGraph, SDKs, Python vs JS comparison

## Deep Agents
[Deep Agents README → deepagents/README.md](deepagents/README.md)

Batteries-included, model-agnostic agent harness inspired by Claude Code:
- [Architecture](deepagents/00-architecture.md) — Middleware stack, backend protocol, sub-agent types, summarization, permissions, CLI, deployment

## Deep Agents JS
[Deep Agents JS README → deepagentsjs/README.md](deepagentsjs/README.md)

TypeScript port of the Deep Agents harness:
- [Architecture](deepagentsjs/00-architecture.md) — Same middleware architecture, JS-native

## Agent Protocol
[Agent Protocol README → agent-protocol/README.md](agent-protocol/README.md)

Framework-agnostic API specification for serving LLM agents:
- [Specification](agent-protocol/00-specification.md) — Runs, Threads, Store, streaming protocol (CDDL), server implementation

## OpenGPTs
[OpenGPTs README → opengpts/README.md](opengpts/README.md)

Open-source clone of OpenAI's GPTs and Assistants API:
- [Architecture](opengpts/00-architecture.md) — Three bot types, FastAPI backend, React frontend, PostgreSQL + pgvector

## Open Deep Research
[Open Deep Research README → open_deep_research/README.md](open_deep_research/README.md)

Deep research agent ranked #6 on the Deep Research Bench Leaderboard:
- [Architecture](open_deep_research/00-architecture.md) — 4-node workflow with parallel researcher subgraphs, token-limit retry

## Open SWE
[Open SWE README → open-swe/README.md](open-swe/README.md)

Internal coding agent inspired by Stripe Minions — Slack/Linear/GitHub triggered:
- [Architecture](open-swe/00-architecture.md) — Deep Agents-based with 21 tools, 4 middleware, cloud sandboxes

## Social Media Agent
[Social Media Agent README → social-media-agent/README.md](social-media-agent/README.md)

URL → Twitter/LinkedIn post generation with HITL review:
- [Architecture](social-media-agent/00-architecture.md) — 14 graphs for full content pipeline

## Chat LangChain
[Chat LangChain README → chat-langchain/README.md](chat-langchain/README.md)

Documentation assistant for LangChain ecosystem:
- [Architecture](chat-langchain/00-architecture.md) — Docs search tools, guardrails middleware

## Executive AI Assistant
[Executive AI Assistant README → executive-ai-assistant/README.md](executive-ai-assistant/README.md)

Email triage agent — Gmail monitoring, drafting, scheduling:
- [Architecture](executive-ai-assistant/00-architecture.md) — Two graphs (main + cron), triage → HITL → action

## Conductor
[Conductor README → conductor/README.md](conductor/README.md)

Production agent architecture using durable execution:
- Every step is a checkpoint
- DO_WHILE loop with LLM-as-planner
- Human approval as durable gate
- Compensation for side effects

## Stripe
[Stripe README → stripe/README.md](stripe/README.md)

Minions — Stripe's one-shot coding agents at scale:
- 1,300+ PRs/week, no human-written code
- DevBox infrastructure (hot and ready in 10s)
- Blueprint orchestration (deterministic + agent nodes)
- MCP Toolshed (~500 tools, curated per agent)

## Agent Observability
[Observability README → agent-observability/README.md](agent-observability/README.md)

Comprehensive guide to agent observability:
- Runs, traces, threads as core primitives
- LangSmith stack architecture
- Building observability from scratch
- Evaluation methods (deterministic, LLM-as-judge, rubric)
- Production monitoring metrics and dashboards
- The agent improvement loop
- [LangSmith internals](agent-observability/01-langsmith-internals.md) — Source code deep dive (v0.8.0): contextvars tracing, dotted_order encoding, UUIDv7, background thread batching, write replicas, provider wrappers
