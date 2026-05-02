---
title: Rust Authz Crates — Documentation Spec
---

# Rust Authz Crates — Documentation Spec

## Source Codebase

- **Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/src.authz/`
- **Language:** Rust (edition 2024)
- **Projects:** 4 crates/workspaces
  - `authz-core` — Zanzibar-style FGA engine (0.1.0, Apache-2.0)
  - `dbrest` — REST API for PostgreSQL/SQLite (0.11.0, Apache-2.0)
  - `pgauthz` — PostgreSQL extension wrapping authz-core (workspace)
  - `zradar` — Telemetry platform with plugin architecture (workspace)
- **MSRV:** Rust 1.85+ (authz-core), Rust 1.91+ (dbrest)

## What the Projects Are

Four Rust crates that share an authorization and data-services ecosystem:

1. **authz-core** — Database- and transport-agnostic Zanzibar-style fine-grained authorization engine. Provides model parsing (pest-based DSL), graph-walking resolver, CEL condition evaluation, and pluggable caching. No datastore dependency.

2. **pgauthz** — PostgreSQL extension (via pgrx) that exposes authz-core as SQL functions. Implements `TupleReader`/`TupleWriter` via PostgreSQL SPI. Provides `pgauthz_check`, `pgauthz_add_relation`, `pgauthz_define_policy`, and Watch API via changelog.

3. **dbrest** — High-performance REST API layer for PostgreSQL and SQLite databases. Pluggable backend architecture with `DatabaseBackend`/`SqlDialect` traits. Supports JWT authentication, OpenAPI auto-generation, and OpenTelemetry metrics/tracing.

4. **zradar** — Telemetry ingestion and storage platform. Layered plugin architecture (core traits → plugin implementations → services → applications). Supports PostgreSQL, ClickHouse, S3, Redis, and local storage backends. Ingests OTLP traces/metrics/logs.

## Documentation Goal

A reader should understand:

1. How authz-core's model DSL works and how it maps to AST types
2. How the CoreResolver graph-walking algorithm resolves checks
3. How pgauthz integrates authz-core into PostgreSQL as an extension
4. How dbrest's pluggable backend architecture works
5. How zradar's layered plugin architecture is structured
6. How all four projects connect via shared traits and patterns

## Documentation Structure

```
rust-authz/
├── spec.md
├── markdown/
│   ├── README.md
│   ├── 00-overview.md
│   ├── 01-architecture.md
│   ├── 02-authz-core-model.md
│   ├── 03-authz-core-resolver.md
│   ├── 04-authz-core-conditions.md
│   ├── 05-pgauthz.md
│   ├── 06-dbrest.md
│   ├── 07-zradar.md
│   ├── 08-data-flow.md
│   └── 09-cross-cutting.md
├── html/
└── build.py (shared)
```

## Tasks

| # | Task | Status |
|---|------|--------|
| 1 | Write spec.md | DONE |
| 2 | Write README.md (index) | TODO |
| 3 | Write 00-overview.md | TODO |
| 4 | Write 01-architecture.md | TODO |
| 5 | Write 02-authz-core-model.md | TODO |
| 6 | Write 03-authz-core-resolver.md | TODO |
| 7 | Write 04-authz-core-conditions.md | TODO |
| 8 | Write 05-pgauthz.md | TODO |
| 9 | Write 06-dbrest.md | TODO |
| 10 | Write 07-zradar.md | TODO |
| 11 | Write 08-data-flow.md | TODO |
| 12 | Write 09-cross-cutting.md | TODO |
| 13 | Grandfather review — verify names, numbers, flows, coverage | TODO |
| 14 | Generate HTML via build.py | TODO |

## Build System

```bash
cd documentation && python3 build.py rust-authz
```

Shared build script: `documentation/build.py` (Python 3.12+ stdlib, zero dependencies).

## Quality Requirements

Per the markdown_engineering directive — 10 Iron Rules:
1. Detailed sections with code snippets (file path references)
2. Teach key facts quickly — thesis-first paragraphs
3. Clear articulation — one idea per sentence
4. Minimum 2 mermaid diagrams per document
5. Good visual assets (tables, ASCII, code blocks)
6. Generated HTML with consistent navigation
7. Cross-references — no orphan pages
8. Source path references (file:line)
9. Aha moments — non-obvious design insights
10. Navigation: index + prev/next buttons

## Expected Outcome

A developer reading these docs can:
- Write an authz model DSL, parse it, and evaluate checks
- Deploy pgauthz as a PostgreSQL extension
- Run dbrest as a standalone REST API server
- Understand zradar's plugin architecture and extend it

## Resume Point

All documents are in `documentation/rust-authz/markdown/`. Each document lists next doc at the end. Spec.md tracks task completion.
