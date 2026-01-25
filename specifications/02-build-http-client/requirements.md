---
description: Create an HTTP 1.1 client using existing simple_http module structures with iterator-based patterns and valtron executors
status: in-progress
priority: high
created: 2026-01-18
author: Main Agent
metadata:
  version: '4.0'
  last_updated: 2026-01-25
  estimated_effort: large
  tags:
    - http-client
    - networking
    - rust
    - iterator-patterns
    - valtron-executors
  stack_files:
    - .agents/stacks/rust.md
  skills: []
  tools:
    - Rust
    - cargo
builds_on:
  - ../04-condvar-primitives
related_specs:
  - ../03-wasm-friendly-sync-primitives
has_features: true
has_fundamentals: true # HTTP client needs comprehensive user documentation
features:
  completed: 0
  uncompleted: 13
  total: 13
  completion_percentage: 0
files_required:
  main_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/05-coding-practice-agent-orchestration.md
      - .agents/rules/06-specifications-and-requirements.md
    files:
      - ./requirements.md
      - ./LEARNINGS.md
      - ./PROGRESS.md

  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/stacks/rust.md
    files:
      - ./requirements.md

  # NOTE: No implementation_agent section - they load feature.md files directly
  # Implementation agents read: ./features/[feature-name]/feature.md (per feature's files_required)
---

# HTTP 1.1 Client - Requirements

> **Specification Structure**: has_features: true → This file is HIGH-LEVEL OVERVIEW ONLY. Detailed requirements and tasks are in `features/*/feature.md` files.

## Overview

Create an HTTP 1.1 client using existing `simple_http` module structures, leveraging iterator-based patterns and valtron executors. This is a feature-based specification with 13 features organized by dependency.

**Key Approach**:
- Iterator-based patterns via `TaskIterator` trait (no async/await)
- Valtron's `single` and `multi` executor modules for execution
- Generic types for flexibility (not boxed)
- Pluggable DNS resolution
- Optional connection pooling and redirect handling

---

## Requirements Conversation Summary

### User's Initial Request

Build an HTTP 1.1 client that:
- Uses existing `simple_http` module structures
- Leverages iterator patterns from `valtron` for non-blocking streaming
- Uses valtron's `single` and `multi` executor modules
- Avoids async/await - purely synchronous with iterator-based streaming
- Supports pluggable DNS resolution, configurable connection pooling, redirect handling
- Reuses TLS infrastructure from `netcap`

### Clarifying Questions Asked

1. **Connection Pooling**: Optional/configurable - support both modes
2. **Redirect Handling**: Configurable - user sets max redirects or disables
3. **DNS Resolution**: Pluggable - custom resolver trait with implementations
4. **Code Location**: `wire/simple_http/client/` submodule
5. **Error Handling**: Custom errors using `derive_more::From`, `Debug`, `Display`
6. **Type Flexibility**: Generic types (e.g., `<T: StreamIterator>`)
7. **Execution Model**: Use valtron's `multi` and `single` modules via `TaskIterator`

### Final Requirements Agreement

- HTTP 1.1 client using existing `simple_http` structures
- Iterator-based patterns via `TaskIterator` trait internally
- Feature-gated executor selection: `multi` feature for multi-threaded
- Custom error types with proper traits
- Generic type parameters (not boxed) for flexibility
- Pluggable DNS, optional pooling, configurable redirects
- TLS support via existing `netcap` infrastructure

---

## Feature Index

**Purpose**: Directory of all features with dependencies. Agents load specific feature.md files as needed.

### Core Features (Required)

| # | Feature | Description | Dependencies | Status |
|---|---------|-------------|--------------|--------|
| 0 | [valtron-utilities](./features/valtron-utilities/feature.md) | Reusable ExecutionAction types, unified executor, state machine helpers | None | ⬜ Pending |
| 1 | [tls-verification](./features/tls-verification/feature.md) | Verify and fix TLS backends (rustls, openssl, native-tls) | 0 | ⬜ Pending |
| 2 | [foundation](./features/foundation/feature.md) | Error types and DNS resolution | 1 | ⬜ Pending |
| 3 | [compression](./features/compression/feature.md) | gzip, deflate, brotli support | 2 | ⬜ Pending |
| 4 | [connection](./features/connection/feature.md) | URL parsing, TCP, TLS | 2 | ⬜ Pending |
| 5 | [proxy-support](./features/proxy-support/feature.md) | HTTP/HTTPS/SOCKS5 proxy | 4 | ⬜ Pending |
| 6 | [request-response](./features/request-response/feature.md) | Request builder, response types | 4 | ⬜ Pending |
| 7 | [auth-helpers](./features/auth-helpers/feature.md) | Basic, Bearer, Digest auth | 6 | ⬜ Pending |
| 8 | [task-iterator](./features/task-iterator/feature.md) | TaskIterator, ExecutionAction, executors | 0, 6 | ⬜ Pending |
| 9 | [public-api](./features/public-api/feature.md) | User-facing API, SimpleHttpClient, integration | 8 | ⬜ Pending |

### Extended Features (Optional)

| # | Feature | Description | Dependencies | Status |
|---|---------|-------------|--------------|--------|
| 10 | [cookie-jar](./features/cookie-jar/feature.md) | Automatic cookie handling | 9 | ⬜ Pending |
| 11 | [middleware](./features/middleware/feature.md) | Request/response interceptors | 9 | ⬜ Pending |
| 12 | [websocket](./features/websocket/feature.md) | WebSocket client and server | 4, 9 | ⬜ Pending |

**Status Key**: ⬜ Pending | 🔄 In Progress | ✅ Complete

**Notes**:
- Features must be implemented in dependency order
- Each feature.md contains detailed requirements, tasks, and verification commands
- Update status in this table as features complete

---

## Success Criteria (Spec-Wide)

**All Features Complete**:
- [ ] All 13 features in index marked complete (✅)
- [ ] All inter-feature integration tests passing
- [ ] Cross-feature functionality verified

**Spec-Wide Quality**:
- [ ] All features pass `cargo clippy -- -D warnings` (zero warnings)
- [ ] All features pass `cargo test --package foundation_core`
- [ ] All features pass `cargo fmt -- --check`
- [ ] No conflicts between features
- [ ] Consistent code quality across all features

**Integration Tests**:
- [ ] End-to-end HTTP requests work across features
- [ ] Connection pooling + TLS + auth work together
- [ ] Compression + streaming work together
- [ ] Proxy + TLS work together

**Documentation**:
- [ ] LEARNINGS.md documents key insights
- [ ] REPORT.md created at completion
- [ ] VERIFICATION.md created with spec-wide verification signoff
- [ ] fundamentals/ directory created with comprehensive user documentation
- [ ] fundamentals/00-overview.md covers HTTP client usage, patterns, and examples

---

## Module Documentation References

Implementation agents MUST read these before making changes:

- **simple_http module**: `documentation/simple_http/doc.md`
- **valtron executors**: `documentation/valtron/doc.md`
- **netcap TLS**: `documentation/netcap/doc.md`

---

_Created: 2026-01-18_
_Last Updated: 2026-01-25 (v4.0 - Restructured to feature-based overview only)_
