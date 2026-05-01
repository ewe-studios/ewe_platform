---
title: Chat LangChain -- Documentation Assistant Agent
---

# Chat LangChain -- Documentation Assistant Agent

## Purpose

A documentation assistant agent that answers questions about LangChain, LangGraph, and LangSmith. Built with LangGraph using a docs-first research strategy.

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/chat-langchain/`

## Architecture

```
User Query --> Guardrails Middleware --> Agent (with tools) --> Response
                                    |
                            +--------+--------+
                            |                 |
                    SearchDocsByLangChain   Pylon KB Tools
                    (Mintlify API)         (Support articles)
```

## Agent (`src/agent/docs_graph.py`)

Uses `langchain.agents.create_agent()` with:
- **Model:** `configurable_model` with fallback middleware
- **Tools:** `SearchDocsByLangChain` (docs search via Mintlify), `search_support_articles` and `get_article_content` (Pylon KB), `check_links` (URL validation)

## Guardrails (`src/middleware/guardrails_middleware.py:1-328`)

`GuardrailsMiddleware` intercepts queries via `abefore_agent` hook (line 186). Uses structured `GuardrailsDecision` Pydantic model with ALLOWED/BLOCKED outcomes. Deliberately permissive — default is to allow, only blocking egregious misuse. Failed classification is fail-open (line 315).

## langgraph.json

Maps `docs_agent` to `./src/agent/docs_graph.py:docs_agent`, recursion limit 100, checkpointer with 7-day TTL.

[Back to main index → ../README.md](../README.md)
