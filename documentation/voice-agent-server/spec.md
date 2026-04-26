# Voice Agent Server Documentation -- Project Spec

## Source Codebase

**Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AIApps/voice-agent-server/`
**Language:** TypeScript (strict mode)
**Runtime:** Node.js 22.x
**Framework:** Express 5.x
**License:** ISC

## What Voice Agent Server Is

A lightweight Express.js REST API that manages voice AI assistants and phone numbers. It syncs every write operation to both the Vapi AI platform API and a local JSON file database. Three source files, ~500 lines total.

## Documentation Goal

Create documentation proportional to the project's simplicity. A developer should be able to understand the entire server in 10 minutes.

## Documentation Structure

```
voice-agent-server/
├── spec.md                         ← THIS FILE (project tracker)
├── markdown/                       ← Source documentation (viewable on GitHub)
│   ├── README.md                   ← Index / table of contents
│   ├── 00-overview.md              ← What it is, purpose, architecture
│   ├── 01-api.md                   ← REST API endpoints (assistants, phone numbers)
│   ├── 02-stack.md                 ← Tech stack: Express, Vapi SDK, 11Labs, JSON DB
│   ├── 03-deployment.md            ← Docker, Fly.io, environment variables
│   └── 04-development.md           ← Setup, dev workflow, testing
├── html/                           ← Rendered HTML (viewable locally)
│   ├── index.html                  ← Auto-generated index + navigation
│   ├── styles.css                  ← Shared CSS (dark/light, responsive)
│   └── 00-overview.html ...        ← Auto-generated from markdown
```

## Tasks

### Phase 1: Core Documentation (Markdown)

| # | Task | Status | File |
|---|------|--------|------|
| 1 | README index | DONE | `README.md` |
| 2 | Overview: what it is, purpose, architecture | DONE | `00-overview.md` |
| 3 | API: all REST endpoints documented | DONE | `01-api.md` |
| 4 | Stack: tech choices, external services, DB | DONE | `02-stack.md` |
| 5 | Deployment: Docker, Fly.io, env vars | DONE | `03-deployment.md` |
| 6 | Development: setup, workflow, testing | DONE | `04-development.md` |

### Phase 2: HTML Rendering

| # | Task | Status | File |
|---|------|--------|------|
| 7 | Generate HTML from markdown via build.py | TODO | `html/*.html` |

## Build System

**Script:** `documentation/build.py` (shared with Pi and Hermes)
**Dependencies:** None (Python 3.12+ stdlib only)

**Usage:**

```bash
cd documentation && python3 build.py voice-agent-server
```

**Rebuild:** Run the same command. Overwrites existing HTML. Idempotent.

## Expected Outcome

A developer unfamiliar with the project can:

1. Read the overview and understand what the server does in 5 minutes
2. Use the API documentation to call every endpoint correctly
3. Understand the tech stack and external service dependencies
4. Deploy to Docker or Fly.io
5. Set up a local development environment
