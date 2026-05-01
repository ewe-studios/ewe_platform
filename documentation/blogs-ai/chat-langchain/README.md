# Chat LangChain -- Documentation Assistant

Documentation assistant agent that answers questions about LangChain, LangGraph, and LangSmith.

## Documents

- [00 Architecture](00-architecture.md) — Agent with docs search tools, guardrails middleware, LangGraph deployment config

## Architecture

```
User Query --> Guardrails --> Agent (SearchDocs, Pylon KB, CheckLinks) --> Response
```
