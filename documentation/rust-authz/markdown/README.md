---
title: rust-authz — Documentation Index
---

# rust-authz Documentation

Four Rust crates covering fine-grained authorization, REST database APIs, and telemetry ingestion.

## Getting Started

Start here: [00-overview.md](markdown/00-overview.md) — high-level introduction to all four projects and how they connect.

## Documents

### Foundation

- **[00-overview](00-overview.html)** — Project overview, how the four crates connect, key numbers
- **[01-architecture](01-architecture.html)** — Dependency graphs, layer diagrams, crate structures for all four projects
- **[08-data-flow](08-data-flow.html)** — 8 end-to-end sequence diagrams covering authorization checks, policy writes, REST queries, OTLP ingestion, and cache invalidation
- **[09-cross-cutting](09-cross-cutting.html)** — Error handling patterns, observability (tracing/metrics), testing approaches, release profiles, deployment

### authz-core — Authorization Engine

- **[02-authz-core-model](02-authz-core-model.html)** — Model DSL grammar, pest parser, AST types, semantic validation (6 checks), TypeSystem queries, cycle detection via DFS
- **[03-authz-core-resolver](03-authz-core-resolver.html)** — CoreResolver implementation, resolution algorithms for all 6 expression types, L2/L3 caching, recursion strategies, concurrency control
- **[04-authz-core-conditions](04-authz-core-conditions.html)** — CEL conditions for attribute-based access control, compilation, evaluation, missing parameter detection

### pgauthz — PostgreSQL Extension

- **[05-pgauthz](05-pgauthz.html)** — pgrx extension details, SQL functions (check/policy/watch), GUC configuration, tracing bridge (tracing → ereport), matrix testing framework

### dbrest — REST API for Databases

- **[06-dbrest](06-dbrest.html)** — Schema introspection, pluggable backend pattern (DatabaseBackend + SqlDialect traits), request pipeline (ApiRequest → action_plan → query generation), JWT auth, OpenAPI generation, benchmarks

### zradar — Telemetry Platform

- **[07-zradar](07-zradar.html)** — Four-layer plugin architecture, OTLP ingestion, pluggable storage (PostgreSQL/ClickHouse/S3/Redis/local), DataFusion analytics, dual rlib/cdylib plugin builds

## Quick Reference

| Project | Type | Key Dependencies | Deployment |
|---------|------|-----------------|------------|
| authz-core | Library (no feature flags) | pest, CEL, moka | Linked into pgauthz |
| pgauthz | PostgreSQL extension | pgrx, authz-core | `.so` into PG lib/ |
| dbrest | Standalone binary | axum, sqlx | Single binary |
| zradar | Multi-service platform | tonic, ClickHouse, DataFusion | server + worker binaries |

## Source Locations

All source code lives under `/home/darkvoid/Boxxed/@formulas/src.rust/src.auth/src.authz/`:

```
authz-core/     — Core authorization engine (model DSL, resolver, CEL conditions)
pgauthz/        — PostgreSQL extension (pgrx, SQL functions, metrics, tracing bridge)
dbrest/         — REST API for PostgreSQL/SQLite (axum, sqlx, schema introspection)
zradar/         — Telemetry platform (OTLP ingestion, plugin architecture, ClickHouse)
```
