# Paperclip -- Documentation Index

**Paperclip** is open-source orchestration for zero-human companies. A Node.js server and React UI that orchestrates teams of AI agents to run a business. Bring your own agents, assign goals, and track work and costs from one dashboard.

## Documents

### Foundation

| Document | What It Covers |
|----------|---------------|
| [00-overview.md](./00-overview.md) | What Paperclip is, philosophy, comparison table, problem/solution |
| [01-architecture.md](./01-architecture.md) | Project structure, packages, tech stack, data flow |

### Core Systems

| Document | What It Covers |
|----------|---------------|
| [02-orchestration.md](./02-orchestration.md) | Org charts, roles, heartbeats, task delegation, agent adapters |
| [03-goals-budgets.md](./03-goals-budgets.md) | Goal alignment, budget enforcement, governance, board powers |
| [04-agents-integration.md](./04-agents-integration.md) | BYOA, adapter types, integration levels, heartbeat protocol |
| [05-ticket-system.md](./05-ticket-system.md) | Task hierarchy, audit log, tracing, atomic checkout |

### Ecosystem

| Document | What It Covers |
|----------|---------------|
| [06-subprojects.md](./06-subprojects.md) | Clipmart, PR Reviewer, Companies Tool, Paperclip Website |
| [07-development.md](./07-development.md) | Setup, roadmap, telemetry, contributing, Docker, worktrees |

## Quick Orientation

```
Board (Human)
  │
  ├── Org Chart ──→ Agents (CEO, CTO, Engineers, ...)
  │                    │
  │                    ├── Heartbeats ──→ Scheduled wake-ups
  │                    ├── Tickets    ──→ Task assignment + comments
  │                    └── Budgets    ──→ Cost tracking + enforcement
  │
  ├── Dashboard  ──→ Metrics, costs, burn rate
  └── Governance ──→ Approvals, overrides, pause/resume
```

## Source

`/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AIApps/src.paperclip/paperclip/`
