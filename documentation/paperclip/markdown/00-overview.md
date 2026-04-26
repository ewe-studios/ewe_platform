---
title: Paperclip -- Overview
---

# Paperclip -- Overview

## What Paperclip Is

Paperclip is an open-source orchestration platform for zero-human companies. It is a Node.js server and React UI that orchestrates a team of AI agents to run a business. Bring your own agents, assign goals, and track work and costs from one dashboard.

The tagline says it all: **if OpenClaw is an employee, Paperclip is the company.**

Paperclip looks like a task manager -- but under the hood it has org charts, budgets, governance, goal alignment, and agent coordination. It manages business goals, not pull requests.

## Core Philosophy

Paperclip is not an agent framework. It does not tell you how to build agents. It tells you how to run a company made of them.

| Principle | Description |
|-----------|-------------|
| **Company is the unit of organization** | Everything lives under a Company -- agents, tasks, budgets, goals |
| **Tasks are the communication channel** | All agent communication flows through tasks and comments. No side channels |
| **All work traces to the goal** | Hierarchical task management -- nothing exists in isolation |
| **Board governs** | Humans retain control through the Board with pause, approve, and override powers |
| **Surface problems, do not hide them** | Good auditing and visibility. No silent auto-recovery |
| **Atomic ownership** | Single assignee per task. Atomic checkout prevents conflicts |
| **Progressive deployment** | Trivial to start local, straightforward to scale to hosted |
| **Extensible core** | Clean boundaries so plugins add capabilities without modifying core |

## Three-Step Workflow

| Step | Action | Example |
|------|--------|---------|
| **01** | Define the goal | "Build the #1 AI note-taking app to $1M MRR." |
| **02** | Hire the team | CEO, CTO, engineers, designers, marketers -- any bot, any provider |
| **03** | Approve and run | Review strategy, set budgets, hit go, monitor from the dashboard |

## Problems Paperclip Solves

| Without Paperclip | With Paperclip |
|-------------------|----------------|
| 20 Claude Code tabs open, can not track which does what. On reboot you lose everything | Tasks are ticket-based, conversations are threaded, sessions persist across reboots |
| Manually gathering context from several places to remind your bot what you are doing | Context flows from the task up through the project and company goals |
| Folders of agent configs are disorganized, re-inventing task management | Org charts, ticketing, delegation, and governance out of the box |
| Runaway loops waste hundreds of dollars of tokens before you notice | Cost tracking surfaces token budgets and throttles agents when they are out |
| Recurring jobs require manual kick-off | Heartbeats handle regular work on a schedule |
| Must find your repo, fire up an agent, keep a tab open, babysit it | Add a task in Paperclip. Your coding agent works on it until done |

## What Paperclip Is Not

| Not | Explanation |
|-----|-------------|
| **Not a chatbot** | Agents have jobs, not chat windows |
| **Not an agent framework** | We orchestrate agents; we do not build them |
| **Not a workflow builder** | No drag-and-drop pipelines. Paperclip models companies with org charts, goals, budgets |
| **Not a prompt manager** | Agents bring their own prompts, models, and runtimes |
| **Not a single-agent tool** | For teams. One agent does not need Paperclip. Twenty agents definitely do |
| **Not a code review tool** | Paperclip orchestrates work, not pull requests |

## Agent Compatibility

Paperclip works with any agent that can receive a heartbeat:

| Agent | Integration |
|-------|-------------|
| OpenClaw | Gateway API, heartbeat worker |
| Claude Code | Local process, heartbeat worker |
| Codex | Local process, heartbeat worker |
| Cursor | Cursor API/CLI bridge |
| Bash | Generic process adapter |
| HTTP | Generic HTTP webhook adapter |
| Pi | Local process, heartbeat worker |
| Hermes | Local process, heartbeat worker |

If it can receive a heartbeat, it is hired.

## Comparison Table

| Feature | Paperclip | Agent Frameworks | Task Managers |
|---------|-----------|------------------|---------------|
| **Orchestration** | Multi-agent coordination | Single-agent runtime | Human task assignment |
| **Org structure** | Hierarchical with reporting lines | None | None or flat teams |
| **Budget enforcement** | Per-agent, per-task, per-company cost ceilings | None | None |
| **Governance** | Board approval gates, pause/resume, rollback | None | Manual review |
| **Agent types** | Any agent (BYOA) | Framework-specific only | N/A |
| **Persistence** | Embedded Postgres, sessions survive reboots | File-based or none | Database |
| **Multi-company** | Yes, with data isolation | No | Yes |
| **Audit trail** | Full tool-call tracing, immutable log | None or partial | Comment history |
| **Scheduling** | Heartbeat system with cron-like triggers | External or none | External |
| **License** | MIT | Varies | Proprietary or SaaS |

## Technology Stack

| Concern | Choice |
|---------|--------|
| Language | TypeScript (strict) |
| Server | Express (REST API) |
| Frontend | React + Vite |
| Database | PostgreSQL (Drizzle ORM) |
| Auth | Better Auth |
| Package management | pnpm workspaces |
| Test | Vitest + Playwright |
| Runtime | Node.js 20+ |
| License | MIT |

## Quickstart

```bash
npx paperclipai onboard --yes
```

Or manually:

```bash
git clone https://github.com/paperclipai/paperclip.git
cd paperclip
pnpm install
pnpm dev
```

This starts the API server at `http://localhost:3100`. An embedded PostgreSQL database is created automatically.
