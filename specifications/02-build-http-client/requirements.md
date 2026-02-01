---
description: Create an HTTP 1.1 client using existing simple_http module structures with iterator-based patterns and valtron executors
status: in-progress
priority: high
created: 2026-01-18
author: Main Agent
context_optimization: true  # Sub-agents MUST generate COMPACT_CONTEXT.md before work, reload after updates
compact_context_file: ./COMPACT_CONTEXT.md  # Ultra-compact current task context (97% reduction)
context_reload_required: true  # Clear and reload from compact context regularly to prevent context limit errors
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
  completed: 7
  uncompleted: 6
  total: 13
  completion_percentage: 54
files_required:
  main_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/05-coding-practice-agent-orchestration.md
      - .agents/rules/06-specifications-and-requirements.md
      - .agents/rules/14-machine-optimized-prompts.md
      - .agents/rules/15-instruction-compaction.md
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
      - .agents/rules/14-machine-optimized-prompts.md
      - .agents/rules/15-instruction-compaction.md
      - .agents/stacks/rust.md
    files:
      - ./requirements.md

  # NOTE: No implementation_agent section - they load feature.md files directly
  # Implementation agents read: ./features/[feature-name]/feature.md (per feature's files_required)
  # All agents MUST load Rules 14 & 15 for token/context optimization
---

# HTTP 1.1 Client - Requirements

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this specification MUST use retrieval-led reasoning.**

### Before Starting Implementation

**YOU MUST** (in this order):
1. ✅ **Search the codebase** for similar implementations using Grep/Glob
2. ✅ **Read existing code** to understand project patterns and conventions
3. ✅ **Check stack files** (`.agents/stacks/[language].md`) for language-specific patterns
4. ✅ **Read module documentation** for modules you'll modify
5. ✅ **Follow discovered patterns** - do NOT invent new patterns without justification
6. ✅ **Verify all assumptions** by reading actual code

### FORBIDDEN Approaches

**YOU MUST NOT**:
- ❌ Assume typical patterns without checking the codebase
- ❌ Implement without searching for similar code first
- ❌ Apply generic best practices without verifying project conventions
- ❌ Guess file structures, naming conventions, or API patterns
- ❌ Use pretraining knowledge without verification against project code

### Retrieval Examples

**Good Retrieval Approach** ✅:
```
"Let me search for existing API endpoints to understand the pattern..."
→ Uses Grep to find similar endpoints
→ Reads actual implementation files
→ Follows discovered patterns (e.g., Axum with custom middleware)
→ Implements consistently with existing code
```

**Bad Pretraining Approach** ❌:
```
"I'll create an API endpoint using Express middleware (standard approach)"
→ Assumes Express without checking project
→ Doesn't verify actual framework used
→ Creates inconsistent code
```

### Enforcement

- Agents will be asked to demonstrate retrieval steps
- Implementation that doesn't match project patterns will be rejected
- "I assumed..." is NOT acceptable - only "I found..." backed by code references

---

## 🚀 CRITICAL: Token and Context Optimization

**ALL agents implementing this specification MUST follow token and context optimization protocols.**

**MANDATORY RULES**: [Rule 14](./.agents/rules/14-machine-optimized-prompts.md) and [Rule 15](./.agents/rules/15-instruction-compaction.md)

### Machine-Optimized Prompts (Rule 14)

**Reference**: [.agents/rules/14-machine-optimized-prompts.md](../../.agents/rules/14-machine-optimized-prompts.md)

**Main Agent MUST**:
1. Generate `machine_prompt.md` from this file when specification finalized
2. Use pipe-delimited compression (58% token reduction)
3. Commit machine_prompt.md alongside human-readable file
4. Regenerate when human file updates
5. Provide machine_prompt.md path to sub-agents

**Sub-Agents MUST**:
- Read `machine_prompt.md` (NOT verbose human files)
- Parse DOCS_TO_READ section for files to load
- 58% token savings

**File Lifecycle**:
- `requirements.md` (human-readable, permanent) → `machine_prompt.md` (machine-optimized, generated)
- Both files committed together, stay in sync
- Sub-agents use machine_prompt.md for instructions

### Context Compaction (Rule 15)

**Reference**: [.agents/rules/15-instruction-compaction.md](../../.agents/rules/15-instruction-compaction.md)

**Main Agent MUST** (before spawning sub-agents):
1. Generate machine_prompt.md (Rule 14)
2. Clear context and reload from machine_prompt.md
3. Read/create PROGRESS.md
4. Generate initial `COMPACT_CONTEXT.md`:
   - Extract current task from machine_prompt.md
   - EMBED machine_prompt content for current task
   - Create ultra-compact self-contained file (500-800 tokens)
5. Provide COMPACT_CONTEXT.md path to sub-agent

**Sub-Agents MUST** (on startup):
1. Receive COMPACT_CONTEXT.md from Main Agent (already generated)
2. Read COMPACT_CONTEXT.md (self-contained with embedded machine_prompt)
3. Read files from FILES section only
4. Begin work with clean compact context (~5K tokens)

**Sub-Agents MUST** (during work - after PROGRESS.md updates):
1. Regenerate COMPACT_CONTEXT.md:
   - Re-extract current task from machine_prompt.md
   - Re-embed machine_prompt content for current task
   - Update status from new PROGRESS.md
   - Update FILES list and NEXT_ACTIONS
2. CLEAR entire context (drop everything)
3. RELOAD from COMPACT_CONTEXT.md only
4. Continue work with refreshed minimal context
5. Proceed with 97% context reduction (180K→5K tokens)

**COMPACT_CONTEXT.md Lifecycle**:
- Generated fresh per task (Main Agent creates initial, Sub-Agent maintains)
- Contains ONLY current task (no history)
- Embeds machine_prompt.md content (self-contained)
- Regenerated after each PROGRESS.md update
- Deleted when task completes (Main Agent cleanup)
- Rewritten from scratch for next task

**Combined Token Flow**:
```
requirements.md (human, 2000 tokens)
    ↓ [Rule 14: Generate]
machine_prompt.md (machine, 900 tokens, 58% reduction)
    ↓ [Rule 15: Extract + Embed]
COMPACT_CONTEXT.md (ultra-compact, 500 tokens, 97% reduction)
    ↓ [After context clear]
Agent works with 500 tokens + FILES (~5K total)
```

**See Also**:
- [Rule 14: Machine-Optimized Prompts](../../.agents/rules/14-machine-optimized-prompts.md)
- [Rule 15: Instruction Compaction](../../.agents/rules/15-instruction-compaction.md)
- [COMPACT_CONTEXT Template](../../.agents/templates/COMPACT_CONTEXT-template.md)

---

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
| 0 | [valtron-utilities](./features/valtron-utilities/feature.md) | Reusable ExecutionAction types, unified executor, state machine helpers | None | ✅ Complete |
| 1 | [tls-verification](./features/tls-verification/feature.md) | Verify and fix TLS backends (rustls, openssl, native-tls) | 0 | ✅ Complete |
| 2 | [foundation](./features/foundation/feature.md) | Error types and DNS resolution | 1 | ✅ Complete |
| 3 | [compression](./features/compression/feature.md) | gzip, deflate, brotli support | 2 | ⬜ Pending |
| 4 | [connection](./features/connection/feature.md) | URL parsing, TCP, TLS (HTTPS fully working) | 2 | ✅ Complete |
| 5 | [proxy-support](./features/proxy-support/feature.md) | HTTP/HTTPS/SOCKS5 proxy | 4 | ⬜ Pending |
| 6 | [request-response](./features/request-response/feature.md) | Request builder, response types | 4 | ✅ Complete |
| 7 | [auth-helpers](./features/auth-helpers/feature.md) | Basic, Bearer, Digest auth | 6 | ⬜ Pending |
| 8 | [task-iterator](./features/task-iterator/feature.md) | TaskIterator, ExecutionAction, executors (types public) | 0, 6 | ✅ Complete |
| 9 | [public-api](./features/public-api/feature.md) | User-facing API, SimpleHttpClient, integration | 8 | ⬜ Pending (UNBLOCKED) |

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
