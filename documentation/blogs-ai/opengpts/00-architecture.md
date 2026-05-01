---
title: OpenGPTs -- Open-Source GPTs Clone
---

# OpenGPTs -- Open-Source GPTs Clone

## Purpose

OpenGPTs is an open-source clone of OpenAI's GPTs and Assistants API, built by LangChain. Full control over LLM, prompts, tools, vector database, retrieval algorithm, and chat history.

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/opengpts/`

## Aha Moments

**Aha: Three bot types, one codebase.** Assistant (tool-using ReAct agent), RAGBot (retrieval-focused), ChatBot (system-message-only). Each is a different complexity level sharing the same infrastructure.

**Aha: LangGraph as the execution engine.** The assistant uses a `MessageGraph` — a LangGraph graph where the state is just the message list. The ReAct loop is: LLM decides tool call → execute → loop until no more tool calls.

## Architecture

```
Frontend (React/TS, port 5173) <---> Backend (FastAPI/Python, port 8100) <---> PostgreSQL + pgvector
```

## Backend (`backend/`)

### Agent Dispatcher (`app/agent.py:78-371`)

Three bot types via `ConfigurableAgent`, `ConfigurableChatBot`, `ConfigurableRetrieval`:

| Bot Type | Architecture | LLM Support |
|----------|-------------|-------------|
| **Assistant** | ReAct with LangGraph MessageGraph | GPT-3.5, GPT-4, GPT-4o, Azure, Claude 2, Bedrock, Gemini, Ollama |
| **RAGBot** | Always retrieves, passes docs in system message | Same as Assistant |
| **ChatBot** | Single LLM call with persona | All models |

### Tool Registry (`app/tools.py:46-325`)

12 available tools: DuckDuckGo, Tavily, TavilyAnswer, You.com, Arxiv, PubMed, Wikipedia, SEC Filings, Press Releases, DALL-E, Connery, Sema4.ai Action Server, plus file Retrieval.

### Storage (`app/storage.py:1-269`)

PostgreSQL-backed storage for assistants, threads, users, and checkpoint history. Uses `asyncpg` connection pool.

### REST API (`app/api/`)

| Endpoint | Purpose |
|----------|---------|
| `assistants.py` | CRUD for assistants (list, get, create, update, delete, list public) |
| `threads.py` | Thread management |
| `runs.py:70-90` | Create runs, stream runs via SSE, LangSmith feedback |

### Checkpoint (`app/checkpoint.py`)

`AsyncPostgresCheckpoint` for LangGraph state persistence.

### LLM Factory (`app/llms.py`)

Factory for OpenAI, Anthropic, Google, Fireworks Mixtral, Ollama, Bedrock.

## Frontend (`frontend/`)

React 18, TypeScript, Vite, TailwindCSS, React Router.

Key components:
- `App.tsx:21-189` — main component managing chat list, config list, stream state
- `Chat.tsx`, `ChatList.tsx`, `Config.tsx`, `ConfigList.tsx`, `Message.tsx`, `Tool.tsx`
- Hooks: `useChatList`, `useChatMessages`, `useConfigList`, `useStreamState` (SSE), `useThreadAndAssistant`

## Docker

4 services: `postgres` (pgvector/pg16), `postgres-setup` (migrations), `backend` (port 8100), `frontend` (port 5173).

## Auth

- Dev: cookie-based mock auth (`opengpts_user_id` cookie)
- Production: JWT via OIDC or Local (`AUTH_TYPE=jwt_oidc` or `jwt_local`)

[Back to main index → ../README.md](../README.md)
