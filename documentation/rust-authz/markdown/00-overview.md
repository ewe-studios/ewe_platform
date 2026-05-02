---
title: Overview — What These Crates Are and Why They Exist
---

# Overview — Rust Authz Crates

Four Rust crates form an authorization and data-services ecosystem. They share traits, patterns, and a design philosophy: **database-agnostic cores with pluggable backends**.

## The Four Projects

### authz-core

A database- and transport-agnostic Zanzibar-style fine-grained authorization engine. It parses a custom model DSL into an AST, validates it semantically, and resolves permission checks by walking the model graph. It has **zero datastore dependencies** — all data access goes through the `TupleReader` trait.

Source: `authz-core/src/lib.rs` — 14 public modules, no feature flags.

### pgauthz

A PostgreSQL extension (via pgrx) that wraps authz-core as SQL functions. It implements `TupleReader`, `TupleWriter`, `PolicyReader`, `PolicyWriter`, and `RevisionReader` using PostgreSQL's SPI interface. The extension exposes functions like `pgauthz_check()`, `pgauthz_add_relation()`, and `pgauthz_define_policy()` directly in SQL.

Source: `pgauthz/crates/pgauthz/src/lib.rs` — pgrx extension with GUC configuration, caching, metrics, and OpenTelemetry.

### dbrest

A high-performance REST API layer for PostgreSQL and SQLite databases. It auto-generates REST endpoints from database schema introspection. The architecture is pluggable: a `DatabaseBackend` trait and `SqlDialect` trait separate the core from database-specific implementations.

Source: `dbrest/src/main.rs` — binary crate that re-exports `dbrest-core`, `dbrest-postgres`, and `dbrest-sqlite`.

### zradar

A telemetry ingestion and storage platform with a layered plugin architecture. It ingests OpenTelemetry Protocol (OTLP) traces, metrics, and logs, then stores them in pluggable backends (PostgreSQL, ClickHouse, S3, Redis, local filesystem). The architecture has four layers: core traits, plugin implementations, business logic services, and application binaries.

Source: `zradar/Cargo.toml` — workspace with 15 member crates across 4 layers.

## How They Connect

```mermaid
flowchart LR
    A[authz-core] --> B[pgauthz]
    A -.traits.-.> C[dbrest]
    A -.pattern.-.> D[zradar]
    B -.pgrx/SPI.-.> PG[(PostgreSQL)]
    C -.sqlx.-.> PG
    C -.sqlx.-.> SQ[(SQLite)]
    D -.plugins.-.> PG
    D -.plugins.-.> CH[(ClickHouse)]
    D -.plugins.-.> S3[(S3)]
```

The connection is architectural, not just dependency-based:

- **pgauthz** directly depends on authz-core and implements its datastore traits via PostgreSQL SPI
- **dbrest** shares the same pluggable-backend pattern (traits + concrete implementations)
- **zradar** extends the pattern further with a full plugin system (dynamic loading, multiple storage backends)

## Key Numbers

| Metric | Value | Source |
|--------|-------|--------|
| authz-core modules | 14 | `authz-core/src/lib.rs:97-109` |
| authz-core MSRV | Rust 1.85 | `authz-core/Cargo.toml` |
| dbrest MSRV | Rust 1.91 | `dbrest/Cargo.toml` |
| dbrest sub-crates | 3 (core, postgres, sqlite) | `dbrest/Cargo.toml` |
| zradar workspace crates | 15 | `zradar/Cargo.toml` |
| zradar plugin backends | 5 (postgres, clickhouse, s3, redis, local) | `zradar/Cargo.toml` |
| Default max recursion depth | 25 | `authz-core/src/resolver.rs:38` |
| Default max concurrent dispatches | 50 | `authz-core/src/core_resolver.rs:85` |
| CEL library version | cel 0.12.0 | `authz-core/Cargo.toml` |
| dbrest web framework | axum 0.7 | `dbrest/Cargo.toml` |
| zradar gRPC framework | tonic 0.9 | `zradar/Cargo.toml` |

## Aha: The Trait Pattern Unifies Everything

All four projects share the same architectural principle: **define traits for the abstract interface, implement them concretely per backend, depend on traits not implementations**. authz-core defines `TupleReader`/`TupleWriter` — pgauthz implements them via SPI. dbrest defines `DatabaseBackend`/`SqlDialect` — postgres and sqlite crates implement them. zradar defines repository and storage traits — five plugin crates implement them. This pattern lets the core logic stay database-agnostic while each backend optimizes for its specific strengths.

## What to Read Next

Start with [01-architecture.md](01-architecture.md) for the full dependency graph and layer diagram, then dive into authz-core with [02-authz-core-model.md](02-authz-core-model.md).
