# Blogs AI -- Industry Research Synthesis

Core ideas, principles, and aha moments from leading AI engineering blogs.

## LangChain
[LangChain README → langchain/README.md](langchain/README.md)

Five documents covering LangChain's evolution from chains to the LangGraph/Deep Agents/LangSmith ecosystem:
- Core principles, six production agent features, harness-memory coupling
- LangGraph design: Pregel/BSP, checkpointing, streaming, human-in-the-loop
- Agent harness anatomy: middleware, skills, subagents, context compression
- Observability & evaluation: runs/traces/threads, eval levels, improvement loop
- Production deep agents: runtime, deployment, authorization, security

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
