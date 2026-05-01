# Stripe Minions -- One-Shot Coding Agents

Unattended coding agents at scale — 1,300+ PRs/week with no human-written code.

## Documents

- [00 Minions Architecture](00-minions.md) — DevBox infrastructure, blueprint orchestration, MCP Toolshed, context gathering, feedback loops

## Core Principles

1. **What's good for humans is good for agents.** Devboxes, rule files, pre-push hooks — all built for humans, repurposed for agents.
2. **Blueprints interleave deterministic + agent nodes.** LLMs in contained boxes, deterministic for what you can anticipate.
3. **One or two CI rounds, then done.** Diminishing returns after two full CI loops.
4. **MCP is the capability layer.** ~500 tools, curate per agent.

## Quick Architecture

```
Engineer → Slack → Minion → DevBox (10s provision)
  → Blueprint: Agent nodes (implement) + Deterministic nodes (lint, push)
  → Pre-push linters (< 5s) → CI (3M+ tests) → Autofixes
  → PR ready for review
```
