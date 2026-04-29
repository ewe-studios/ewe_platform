# Resonate -- Documentation Index

**Resonate** is a durable execution engine implementing the Distributed Async Await specification. Write normal async functions in TypeScript, Rust, or Python. Resonate handles retries, crash recovery, and distributed coordination — your code survives process restarts without missing a beat.

## Documents

### Foundation

| Document | What It Covers |
|----------|---------------|
| [00-overview.md](./00-overview.md) | What Resonate is, philosophy, the component map |
| [01-architecture.md](./01-architecture.md) | System architecture, dependency graph, communication patterns |

### Core Concepts

| Document | What It Covers |
|----------|---------------|
| [03-durable-promises.md](./03-durable-promises.md) | Promise lifecycle, states, callbacks, listeners, settlement chains |
| [13-data-flow.md](./13-data-flow.md) | End-to-end execution flows: invoke, suspend, resume, settle |

### Server Internals

| Document | What It Covers |
|----------|---------------|
| [02-server.md](./02-server.md) | Oracle, HTTP API, CLI commands, configuration |
| [07-transport-system.md](./07-transport-system.md) | HTTP push/poll, GCP Pub/Sub, bash exec transports |
| [08-persistence.md](./08-persistence.md) | SQLite, PostgreSQL, MySQL backends and schema |

### SDK Deep Dives

| Document | SDK | Execution Model |
|----------|-----|-----------------|
| [04-sdk-typescript.md](./04-sdk-typescript.md) | `@resonatehq/sdk` | Generator functions (`yield*`) |
| [05-sdk-rust.md](./05-sdk-rust.md) | `resonate-sdk` | Async/await + proc macros |
| [06-sdk-python.md](./06-sdk-python.md) | `resonate` (PyPI) | Generator functions + threading |

### Patterns & Integrations

| Document | What It Covers |
|----------|---------------|
| [09-patterns.md](./09-patterns.md) | Saga, fan-out, human-in-the-loop, external SoR, state bus |
| [10-faas-serverless.md](./10-faas-serverless.md) | AWS Lambda, Cloudflare Workers, Supabase Edge Functions |

### Operations

| Document | What It Covers |
|----------|---------------|
| [11-observability.md](./11-observability.md) | Prometheus metrics, OpenTelemetry, structured tracing |
| [12-deployment.md](./12-deployment.md) | Production setup, JWT auth, configuration layering |

## Quick Orientation

```
┌─────────────────────────────────────────────────────────────────┐
│                          SDKs                                    │
│                                                                  │
│  resonate-sdk-ts       resonate-sdk-rs       resonate-sdk-py    │
│  TypeScript (npm)      Rust (crates.io)      Python (PyPI)      │
│  Generator-based       Async/Await           Generator + Thread  │
│                                                                  │
├─────────────────────────────────────────────────────────────────┤
│                      PROTOCOL (HTTP/JSON)                        │
│                                                                  │
│  RequestEnvelope → Server → ResponseEnvelope                    │
│  Poll (SSE) | Push (webhook) | Pub/Sub | Bash                   │
│                                                                  │
├─────────────────────────────────────────────────────────────────┤
│                     RESONATE SERVER                              │
│                                                                  │
│  Oracle          Persistence       Transports       Processing  │
│  State machine   SQLite/PG/MySQL   HTTP/Poll/GCP    Timeouts    │
│  30 operations   12 tables         4 schemes        Messages    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Source

`/home/darkvoid/Boxxed/@formulas/src.rust/src.resonatehq/`
