---
title: "Pi Extensions -- pi-foreground-chains"
---

# pi-foreground-chains

**Observable multi-agent workflows with full user visibility.**

pi-foreground-chains orchestrates multi-agent workflows with full user visibility. Each step runs in an observable overlay where the user watches and can intervene.

## What It Does

Chains multiple AI agents together with file-based handoff:

```
Scout ──► context.md ──► Planner ──► plan.md ──► Worker ──► impl.md ──► Reviewer
                                                                            │
                                                                      progress.md
                                                                    (complete history)
```

Each agent runs in a hands-free overlay. The user watches in real-time and can take over anytime.

## Agent Roles

| Role | Input | Output |
|------|-------|--------|
| **Scout** | User request | `context.md` (research findings) |
| **Planner** | `context.md` | `plan.md` (implementation plan) |
| **Worker** | `plan.md` | `impl.md` (implementation) |
| **Reviewer** | `impl.md` + `progress.md` | Approval or rework request |

## Why "Foreground"

Unlike background agents that work invisibly, foreground chains run in observable overlays. You see each agent working in real-time and can intervene at any point.

## Requirements

Requires [pi-interactive-shell](13-ext-pi-interactive-shell.md) extension.

## Install

```bash
pi install npm:pi-foreground-chains
```

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-foreground-chains` |
| Requires | pi-interactive-shell |
| Pattern | File-based agent handoff chain |
