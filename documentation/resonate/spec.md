# Resonate Documentation -- Project Spec

## Source Codebase

**Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.resonatehq/`
**Languages:** Rust (server, SDK), TypeScript (SDK, integrations), Python (SDK)
**Server Version:** 0.9.5
**Organization:** ResonateHQ (@resonatehq)
**License:** Apache-2.0

## What Resonate Is

Resonate is a durable execution engine implementing the Distributed Async Await specification. It provides reliable, distributed function execution that survives process restarts and failures. Developers write normal async functions; Resonate handles retries, crash recovery, and distributed coordination automatically.

The system consists of:
- A single-binary **server** (Rust) that persists execution state via durable promises
- **SDKs** in TypeScript, Rust, and Python that integrate durable execution into application code
- **Transports** for delivering execution messages (HTTP push/poll, GCP Pub/Sub, bash)
- **FaaS integrations** for AWS Lambda, Cloudflare Workers, and Supabase Edge Functions

## Documentation Goal

Create comprehensive documentation that lets any developer understand:
1. The Distributed Async Await paradigm and why it exists
2. How the server works internally (oracle, persistence, transports)
3. How each SDK implements the execution model
4. The design patterns that Resonate enables (saga, fan-out, HITL, etc.)
5. How to deploy and operate the server in production
6. How the data flows during function invocation, suspension, and resumption

## Documentation Structure

```
resonate/
├── spec.md                         ← THIS FILE (project tracker)
├── markdown/                       ← Source documentation (viewable on GitHub)
│   ├── README.md                   ← Index / table of contents
│   ├── 00-overview.md              ← What Resonate is, philosophy, component map
│   ├── 01-architecture.md          ← System architecture, component graph, data flow
│   ├── 02-server.md                ← Server internals: oracle, HTTP API, CLI
│   ├── 03-durable-promises.md      ← Core concept: promise lifecycle, states, callbacks
│   ├── 04-sdk-typescript.md        ← TypeScript SDK deep dive
│   ├── 05-sdk-rust.md              ← Rust SDK deep dive
│   ├── 06-sdk-python.md            ← Python SDK deep dive
│   ├── 07-transport-system.md      ← Transport layer: HTTP push/poll, Pub/Sub, bash
│   ├── 08-persistence.md           ← Storage backends: SQLite, PostgreSQL, MySQL
│   ├── 09-patterns.md              ← Design patterns: saga, fan-out, HITL, scheduled work
│   ├── 10-faas-serverless.md       ← FaaS integrations: Lambda, Cloudflare, Supabase
│   ├── 11-observability.md         ← Metrics, OpenTelemetry, tracing
│   ├── 12-deployment.md            ← Server deployment, auth, configuration
│   └── 13-data-flow.md             ← End-to-end execution flows with sequence diagrams
├── html/                           ← Rendered HTML (viewable locally + GitHub Pages)
│   ├── index.html                  ← Auto-generated index + navigation
│   ├── styles.css                  ← Shared CSS (dark/light, responsive)
│   └── 00-overview.html ...        ← Auto-generated from markdown
└── build.py                        ← Markdown → HTML (Python stdlib, zero deps)
```

## Tasks

### Phase 1: Core Documentation (Markdown)

| # | Task | Status | File |
|---|------|--------|------|
| 1 | Overview: what Resonate is, philosophy, component map | DONE | `00-overview.md` |
| 2 | Architecture: system graph, communication patterns | DONE | `01-architecture.md` |
| 3 | Server: oracle, HTTP API, CLI, config | DONE | `02-server.md` |
| 4 | Durable Promises: lifecycle, states, callbacks, listeners | DONE | `03-durable-promises.md` |
| 5 | TypeScript SDK: generators, coroutines, computation | DONE | `04-sdk-typescript.md` |
| 6 | Rust SDK: async/await, context, macros, effects | DONE | `05-sdk-rust.md` |
| 7 | Python SDK: bridge, coroutines, graph, threading | DONE | `06-sdk-python.md` |
| 8 | Transport System: HTTP push/poll, Pub/Sub, bash exec | DONE | `07-transport-system.md` |
| 9 | Persistence: SQLite schema, PostgreSQL, MySQL, trait | DONE | `08-persistence.md` |
| 10 | Patterns: saga, fan-out, HITL, external SoR, state bus | DONE | `09-patterns.md` |
| 11 | FaaS/Serverless: Lambda, Cloudflare, Supabase | DONE | `10-faas-serverless.md` |
| 12 | Observability: Prometheus, OpenTelemetry, tracing | DONE | `11-observability.md` |
| 13 | Deployment: production setup, auth, config layering | DONE | `12-deployment.md` |
| 14 | Data Flow: end-to-end sequences, suspend/resume | DONE | `13-data-flow.md` |
| 15 | README index | DONE | `README.md` |

### Phase 2: HTML Rendering

| # | Task | Status | File |
|---|------|--------|------|
| 16 | Shared CSS (dark/light, code blocks, diagrams) | TODO | `html/styles.css` |
| 17 | Index page with navigation | TODO | `html/index.html` |
| 18 | Build script (markdown → HTML via Python stdlib) | TODO | `../build.py` |
| 19 | Generate all HTML pages | TODO | `html/*.html` |

### Phase 3: Diagrams

| # | Task | Status | Notes |
|---|------|--------|-------|
| 20 | Mermaid rendering in HTML pages | TODO | Client-side via CDN |
| 21 | Theme-aware diagram re-rendering | TODO | Re-renders on dark/light toggle |

## Build System

**Script:** `documentation/build.py` (shared with Pi, Hermes)
**Dependencies:** None (Python 3.12+ stdlib only)

**Usage:**
```bash
cd documentation && python3 build.py resonate
```

## Quality Requirements

All documentation MUST meet these standards:

1. **Detailed sections with code snippets** — Every concept grounded in actual source code
2. **Teach key facts quickly** — Core concept in first paragraph, progressive depth
3. **Clear articulation** — Non-complex sentences, logical flow
4. **Mermaid diagrams** — Minimum 2 diagrams per document
5. **Good visual assets** — Tables, ASCII art, mermaid for complex flows
6. **Cross-references** — Every document links to related documents
7. **Source path references** — Actual file paths from source codebase

## Expected Outcome

A developer unfamiliar with Resonate can:
1. Understand what durable execution means and why it matters in 5 minutes
2. Understand the server architecture and how SDKs communicate with it
3. Deep-dive into any SDK and understand its execution model
4. Follow data flow diagrams for invocation, suspension, and resumption
5. Deploy a production Resonate server with proper auth and monitoring
6. Implement any of the documented patterns in their preferred language
