---
title: aipack — Documentation Index
---

# aipack Documentation

Jeremy Chone's Rust crate collection — 13 crates covering AI providers, JSON-RPC routing, SQL builders, MCP protocol support, and developer tools.

## Getting Started

Start here: [00-overview.md](markdown/00-overview.md) — high-level introduction to all 13 crates and how they connect.

## Documents

### Foundation

- **[00-overview](00-overview.html)** — Crate collection overview, key numbers, crate catalog
- **[01-architecture](01-architecture.html)** — Module structures, dependency diagrams for all crates

### AI Layer

- **[02-genai](02-genai.html)** — Multi-provider AI client (19 providers: OpenAI, Anthropic, Gemini, Ollama, etc.) — chat API, embeddings, tool use, streaming, adapter system
- **[06-agentic](06-agentic.md)** — MCP protocol client/server (HTTP + stdio transports, tools, resources, prompts, sampling, capability negotiation)

### Data Layer

- **[04-sqlb](04-sqlb.html)** — Simple, expressive SQL builder (SELECT/INSERT/UPDATE/DELETE) with sqlx integration
- **[05-modql](05-modql.html)** — Model Query Language (filter operators, JSON filter syntax, SeaQuery/SQLite adapters, pagination)

### Transport Layer

- **[03-rpc-router](03-rpc-router.html)** — JSON-RPC 2.0 router with procedural macros (RpcResource, RpcParams, RpcHandlerError)

### Dev Tools

- **[07-udiffx](07-udiffx.html)** — LLM-optimized unified diff parser/applier with patch completion
- **[08-utilities](08-utilities.html)** — Utility crates: simple-fs (filesystem API), uuid-extra (UUID encoding), pretty-sqlite, dinf, htmd (HTML→Markdown), webdev, webtk

## Quick Reference

| Crate | Version | Files | Purpose |
|-------|---------|-------|---------|
| `genai` | 0.6.0-beta.19 | 110+ | Multi-provider AI client (19 providers) |
| `agentic` | 0.0.5 | 54 | MCP protocol client/server |
| `rpc-router` | 0.2.1 | 32 | JSON-RPC 2.0 router |
| `sqlb` | 0.4.0 | 9 | SQL builder |
| `modql` | 0.5.0-alpha.9 | 40+ | Model Query Language |
| `udiffx` | 0.1.42 | 17 | Unified diff parser |
| `simple-fs` | 0.12.0 | 38 | Filesystem API |
| `uuid-extra` | 0.0.3 | — | UUID base64/base58 encoding |
| `pretty-sqlite` | 0.5.1 | — | SQLite pretty printer |
| `dinf` | 0.1.5 | — | Directory info CLI |
| `htmd` | 0.5.4 | — | HTML to Markdown converter |
| `webdev` | 0.1.1 | — | Local dev web server |
| `webtk` | 0.1.1 | — | Web asset toolkit |

## Source Locations

All source code lives under `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.jeremychone/`:

```
rust-genai/        — Multi-provider AI client library
rust-agentic/      — MCP protocol support
rust-rpc-router/   — JSON-RPC router (+ rpc-router-macros)
rust-sqlb/         — SQL builder (+ sqlb-macros)
rust-modql/        — Model Query Language (+ modql-macros)
rust-udiffx/       — Unified diff parser
rust-simple-fs/    — Filesystem API
rust-uuid-extra/   — UUID encoding utilities
rust-pretty-sqlite/— SQLite pretty printer
rust-dinf/         — Directory info CLI
htmd/              — HTML to Markdown (letmutex)
rust-webdev/       — Local dev server
rust-webtk/        — Web asset toolkit
```
