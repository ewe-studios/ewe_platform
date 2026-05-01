# OpenGPTs -- Open-Source GPTs Clone

Open-source clone of OpenAI's GPTs and Assistants API, built by LangChain.

## Documents

- [00 Architecture](00-architecture.md) — Three bot types (Assistant, RAGBot, ChatBot), FastAPI backend, React frontend, PostgreSQL + pgvector

## Architecture

```
Frontend (React/TS, :5173) <-> Backend (FastAPI, :8100) <-> PostgreSQL + pgvector
```

## Three Bot Types

| Type | Architecture | Purpose |
|------|-------------|---------|
| **Assistant** | ReAct with LangGraph MessageGraph | Tool-using agent with 8 LLM options |
| **RAGBot** | Always retrieves, passes docs in system message | Retrieval-focused |
| **ChatBot** | Single LLM call with persona | Simple conversation |
