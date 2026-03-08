---
purpose: "Entry point for agents implementing the OpenAI-compatible API specification"
version: "1.0"
created: 2026-03-08
---

# OpenAI-Compatible API Specification

This specification implements an OpenAI-compatible API client in `foundation_ai` supporting both Chat Completions and Responses API (reasoning models).

## Quick Start

1. **Read this file** to understand the spec structure
2. **Identify which feature** you need to implement from the Feature Index
3. **Navigate to the feature's start.md** for implementation workflow

## Feature Index

| # | Feature | Status | Start File |
|---|---------|--------|------------|
| 0 | [foundation](./features/00-foundation/feature.md) | Pending | `features/00-foundation/start.md` |
| 1 | [chat-completions-types](./features/01-chat-completions-types/feature.md) | Pending | `features/01-chat-completions-types/start.md` |
| 2 | [chat-completions-client](./features/02-chat-completions-client/feature.md) | Pending | `features/02-chat-completions-client/start.md` |
| 3 | [streaming-support](./features/03-streaming-support/feature.md) | Pending | `features/03-streaming-support/start.md` |
| 4 | [responses-api](./features/04-responses-api/feature.md) | Pending | `features/04-responses-api/start.md` |
| 5 | [integration-tests](./features/05-integration-tests/feature.md) | Pending | `features/05-integration-tests/start.md` |

## Workflow

### For Spec-Level Work

1. Read `requirements.md` for full specification overview
2. Identify which feature needs implementation
3. Navigate to that feature's `start.md` file
4. Follow the feature-level workflow

### For Creating New Features

1. Read `requirements.md` to ensure alignment with spec goals
2. Create feature directory: `features/NN-feature-name/`
3. Create `feature.md` with detailed requirements
4. Create `start.md` for agent workflow
5. Update `requirements.md` feature index

## Files in This Specification

| File | Purpose |
|------|---------|
| `requirements.md` | High-level overview + feature index |
| `start.md` | This file - spec-level entry point |
| `LEARNINGS.md` | Design decisions and learnings (update frequently!) |
| `REPORT.md` | Final implementation report |
| `VERIFICATION.md` | Verification signoff |
| `features/*/feature.md` | Detailed feature requirements |
| `features/*/start.md` | Feature-level workflow entry points |

## Implementation Order

Features must be implemented in dependency order:
1. `00-foundation` (no dependencies)
2. `01-chat-completions-types` (depends on foundation)
3. `02-chat-completions-client` (depends on types)
4. `03-streaming-support` (depends on types)
5. `04-responses-api` (depends on foundation + streaming)
6. `05-integration-tests` (depends on client + responses)

---

**Agents:** For implementation work, always navigate to the specific feature's `start.md` file for detailed workflow instructions.
