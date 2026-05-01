---
title: Open SWE -- Internal Coding Agent
---

# Open SWE -- Internal Coding Agent

## Purpose

Open SWE is an open-source framework for building internal coding agents. Inspired by Stripe Minions, Ramp Inspect, and Coinbase Cloudbot. Triggered from Slack, Linear, or GitHub — runs code changes in isolated cloud sandboxes and opens PRs automatically.

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/open-swe/`

## Aha Moments

**Aha: Built on Deep Agents, not raw LangGraph.** Uses `create_deep_agent()` from the Deep Agents framework with 21 custom tools and 4 middleware hooks — not a ground-up graph.

**Aha: Mid-run message injection.** `check_message_queue_before_model` middleware reads from LangGraph store at namespace `("queue", thread_id)` and injects follow-up messages (Linear comments/Slack replies) that arrived during execution. The agent sees them in its next turn.

**Aha: After-agent PR safety net.** `open_pr_if_needed` middleware commits, pushes, and opens a PR if the agent forgot to do it itself. Never loses work.

## Architecture

```
Slack/Linear/GitHub Webhook --> FastAPI (webapp.py) --> LangGraph Run --> Cloud Sandbox --> Deep Agent --> PR
                                                                                              ↓
                                                                                      Post results back
```

## Core Entry Point (`agent/server.py:190-318`)

`get_agent()` async function:

1. **Sandbox management** (lines 196-268): Creates or reconnects to isolated Linux sandbox. Supports LangSmith, Daytona, Runloop, Modal, and local providers. Auto-recreates on connection failure.
2. **Agent creation** (lines 277-318): Uses `create_deep_agent()` with:
   - **Model:** Default `anthropic:claude-opus-4-6`
   - **System prompt:** Modular sections (working env, task execution, coding standards, PR conventions, code review) + injected `AGENTS.md` from target repo
   - **21 tools:** `http_request`, `fetch_url`, `web_search`, `list_repos`, `get_branch_name`, `commit_and_open_pr`, Linear tools, Slack tools, GitHub tools
   - **4 middleware hooks:** Tool error handler, mid-run message injection, empty message prevention, PR safety net

## Webhook Server (`agent/webapp.py`)

FastAPI server with endpoints for:
- Linear webhook (issue created/updated)
- Slack webhook (mention `@openswe`)
- GitHub webhook (PR review request)

Extracts task + repo info, creates LangGraph run with deterministic thread ID.

## Middleware (`agent/middleware/`)

| Middleware | File | Purpose |
|-----------|------|---------|
| `ToolErrorMiddleware` | `tool_error_handler.py` | Catches tool errors gracefully |
| `check_message_queue_before_model` | `check_message_queue.py:49` | Injects mid-run messages from store |
| `ensure_no_empty_msg` | `ensure_no_empty_msg.py` | Prevents empty messages reaching model |
| `open_pr_if_needed` | `open_pr.py:69` | After-agent PR safety net |

## Sandbox Providers (`agent/integrations/`)

| Provider | File | Purpose |
|----------|------|---------|
| LangSmith Sandbox | `langsmith.py` | Built-in sandbox |
| Daytona | `daytona.py` | Daytona cloud sandbox |
| Runloop | `runloop.py` | Runloop sandbox |
| Modal | `modal.py` | Modal serverless containers |
| Local | `local.py` | Local development |

## Workflow

1. User mentions `@openswe` in Slack, comments on Linear, or tags in GitHub PR
2. Webhook receives event, extracts task + repo info
3. LangGraph run created with deterministic thread ID
4. Cloud sandbox provisioned, repo cloned
5. Agent executes: understand → implement → verify → commit
6. `commit_and_open_pr` tool or `open_pr_if_needed` middleware opens GitHub draft PR
7. Results posted back to invocation surface

[Back to main index → ../README.md](../README.md)
