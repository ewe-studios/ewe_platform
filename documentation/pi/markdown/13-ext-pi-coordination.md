---
title: "Pi Extensions -- pi-coordination"
---

# pi-coordination

**Multi-agent coordination with parallel task execution, dependencies, contracts, and review cycles.**

pi-coordination scales the single-task-per-agent pattern into a team. Independent tasks run in parallel; dependent tasks wait for prerequisites. Workers continuously spawn as tasks become available, and new tasks can be discovered mid-run.

## Philosophy

> "Ralph Wiggum on steroids" -- the same single-task-per-agent pattern, scaled with parallelism and coordination. If Ralph is a solo developer with a todo list, pi-coordination is a team with a project manager, code reviewer, and task board.

Agents work better focused on ONE task with fresh context, rather than juggling an entire spec at once.

## Core Pattern

| Concept | Description |
|---------|-------------|
| **Stateless agents** | Fresh context each time, no accumulated confusion |
| **Stateful files** | `tasks.json` tracks progress, survives crashes |
| **One task per agent** | Focused execution, no context overload |

## What pi-coordination Adds

- **Parallel execution** -- N workers instead of 1
- **Task graph** -- Dependencies, not sequence
- **Review cycles** -- Tasks can be sent back for rework
- **Real-time visibility** -- Watch progress on the task board
- **Contract-based handoff** -- Tasks specify input/output contracts

## Task Graph

A plan is a directed acyclic graph, not a linear list:

```
Task A (research) ──► Task C (design) ──► Task E (implement)
Task B (spike)   ──►┘                     │
                                          ▼
Task D (setup) ───────────────────────► Task F (test)
```

Tasks A and B run in parallel. C waits for both A and B. F waits for E.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-coordination` |
| State file | `tasks.json` |
| Pattern | Task graph with dependencies |
