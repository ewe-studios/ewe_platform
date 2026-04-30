# Graphify -- Documentation Index

Graphify is an AI coding assistant skill that transforms mixed-media corpora -- code, documentation, papers, images, video, and audio -- into queryable knowledge graphs. It ships as a Python library (`graphifyy` on PyPI, v0.5.6) with a CLI entry point and integrates with 14 platforms including Claude Code, Codex, Cursor, and Gemini CLI. This documentation covers the architecture, pipeline stages, security model, and LLM backends.

## Source Code

All source files are located at:
`/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.Graphify/graphify/graphify/`

## Quick Orientation: The Pipeline

```
detect → extract → build → cluster → analyze → report → export
```

Every document in this set describes one or more stages of this pipeline. The pipeline is a linear sequence of pure functions, each in its own module, communicating through plain Python dicts and NetworkX graphs.

## Documents by Category

### Foundation

These documents describe the core pipeline and architecture.

| Document | Covers | Key Modules |
|----------|--------|-------------|
| [Overview](00-overview.md) | Three-pass extraction model, 7-stage pipeline, supported languages | Full pipeline |
| [Data Flow](11-data-flow.md) | Five primary data flows with sequence diagrams: full build, incremental update, graph query, CLI install, MCP server | All pipeline modules |

### Pipeline Deep Dives

These documents go deep into individual pipeline stages.

| Document | Covers | Key Modules |
|----------|--------|-------------|
| [Clustering](05-clustering.md) | Leiden community detection, Louvain fallback, oversized community splitting | `cluster.py` |
| [LLM Backend](12-llm-backend.md) | Direct LLM backends (Claude, Kimi), video transcription, URL ingestion, cost estimation | `llm.py`, `transcribe.py`, `ingest.py` |

### Cross-Cutting

These documents cover concerns that span multiple pipeline stages.

| Document | Covers | Key Modules |
|----------|--------|-------------|
| [Security & Validation](09-security-validation.md) | SSRF protection, DNS rebinding guard, path traversal prevention, extraction JSON validation | `security.py`, `validate.py` |
| [Caching & Performance](10-caching-performance.md) | Per-file SHA256 cache, token-reduction benchmark, filesystem watcher for incremental rebuilds | `cache.py`, `benchmark.py`, `watch.py` |

## Document Details

### [00-overview.md](00-overview.md) -- Overview
The entry point. Describes the three-pass extraction model (AST, Whisper, Claude), the 7-stage pipeline, supported languages (25 via tree-sitter), and the value proposition (71.5x token reduction for mixed corpora).

### [05-clustering.md](05-clustering.md) -- Clustering
Community detection using graspologic's Leiden implementation with Louvain fallback. Covers community splitting for oversized communities (>25% of graph, minimum 10 nodes), the `split_oversized_communities` function, and community label generation.

### [09-security-validation.md](09-security-validation.md) -- Security and Validation
Threat model covering SSRF, DNS rebinding, path traversal, and label injection. Documents `validate_url`, `safe_fetch`, `_ssrf_guarded_socket`, `_NoFileRedirectHandler`, `validate_graph_path`, `sanitize_label`, and `validate_extraction`/`assert_valid`. Includes a full threat model table and data flow diagrams.

### [10-caching-performance.md](10-caching-performance.md) -- Caching and Performance
Per-file SHA256 content cache with separate `ast/` and `semantic/` directories. Token-reduction benchmark measuring corpus tokens vs. graph query tokens. Filesystem watcher with code-only rebuild (no LLM) and non-code change notification.

### [11-data-flow.md](11-data-flow.md) -- Data Flow
Five sequence diagrams tracing data through the system: full graph build, incremental update, graph query, CLI install, and MCP server. Shows actual function names, data shapes, and integration points for caching and security.

### [12-llm-backend.md](12-llm-backend.md) -- LLM Backend
Direct LLM backends for semantic extraction: Claude (claude-sonnet-4-6, $3/$15 per M tokens) and Kimi (kimi-k2.6, $0.74/$4.66). Covers the extraction system prompt, `_call_claude`, `_call_openai_compat`, parallel corpus extraction, cost estimation, Whisper transcription with domain-aware prompts, and URL ingestion with query result persistence.
