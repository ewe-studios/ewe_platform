---
title: Agent Protocol -- Standardized API for LLM Agents
---

# Agent Protocol -- Standardized API for LLM Agents

## Purpose

Agent Protocol is a framework-agnostic API specification for serving LLM agents in production. It defines standardized endpoints around three core concepts: Runs (executions), Threads (multi-turn sessions), and Store (long-term memory).

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/agent-protocol/`

## Aha Moments

**Aha: Unified streaming protocol via CDDL.** The protocol defines a CDDL schema covering all streaming channels — messages, tools, lifecycle events, input requests, values, updates, checkpoints, tasks, custom — with content-block lifecycle states.

**Aha: Three core primitives cover all agent patterns.** Runs (execute), Threads (organize), Store (remember). Everything else — background runs, stateless runs, streaming — are variations on these three.

## API Specification (`openapi.json`)

### Core Primitives

| Concept | Description |
|---------|-------------|
| **Run** | A single agent execution |
| **Thread** | A multi-turn session organizing related runs |
| **Store** | Long-term memory with namespace-based access |

### Endpoint Groups

| Group | Endpoints | Purpose |
|-------|-----------|---------|
| **Stateless Runs** | `POST /runs/wait`, `POST /runs/stream` | Create thread + run in one call |
| **Threads** | `POST /threads`, `GET /threads/{id}`, `PATCH /threads/{id}`, `DELETE /threads/{id}`, `POST /threads/search`, `GET /threads/{id}/history`, `POST /threads/{id}/copy` | Thread CRUD and search |
| **Agents** | `POST /agents/search`, `GET /agents/{id}`, `GET /agents/{id}/schemas` | Agent introspection |
| **Background Runs** | `GET /threads/{id}/runs`, `POST /runs`, `GET /runs/{id}`, `POST /runs/{id}/cancel`, `DELETE /runs/{id}`, `GET /runs/{id}/wait`, `GET /runs/{id}/stream` | Async run management |
| **Store** | `PUT /store/items`, `DELETE /store/items`, `GET /store/items`, `POST /store/items/search`, `POST /store/namespaces` | Long-term memory |
| **Streaming** | `POST /threads/{id}/stream` (SSE), `GET /threads/{id}/stream` (WebSocket), `POST /threads/{id}/commands` | Real-time streaming + HITL |

## Streaming Protocol (`streaming/protocol.cddl`)

Unified streaming protocol with channels:
- `messages` — LLM message content
- `tools` — tool call lifecycle
- `lifecycle` — graph start/end events
- `input` — human-in-the-loop input requests
- `values` — state values after each step
- `updates` — node deltas
- `checkpoints` — checkpoint events
- `tasks` — task start/finish
- `custom` — user-defined events

**Content-block lifecycle:** `message-start` → `content-block-start` → `content-block-delta` × N → `content-block-finish` → `message-finish`

**Tool lifecycle:** `tool-started` → `tool-output-delta` × N → `tool-finished` / `tool-error`

**Human-in-the-loop:** `input.requested` / `input.respond` commands

## Server Implementation (`server/`)

FastAPI-based reference implementation:
- `main.py` — FastAPI app entry point
- `models.py` — Pydantic V2 models for request/response schemas
- `routers/runs.py` — Background run endpoints
- `routers/stateless_runs.py` — Stateless run endpoints
- `routers/threads.py` — Thread CRUD
- `routers/agents.py` — Agent introspection
- `routers/store.py` — Memory store
- `routers/background_runs.py` — Background run management

[Back to main index → ../README.md](../README.md)
