# HTTP 1.1 Client - Progress Report

> **⚠️ EPHEMERAL FILE**: This file tracks CURRENT work only. Cleared after completing each major feature, DELETED when specification complete.

---

## Current Feature: task-iterator

**Status**: In Progress - Starting Implementation
**Started**: 2026-02-01
**Tasks**: 0/11 (0%)

**Progress**: 5/13 features completed (38%)

**Feature Description**:
Internal TaskIterator implementation for HTTP requests/responses, ExecutionAction spawners for child tasks (redirects, TLS upgrades), and feature-gated executor wrapper.

---

## Current Task: Implement TaskIterator Infrastructure

**Objective**: Create internal async machinery using valtron TaskIterator patterns

**Tasks Breakdown**:
1. [ ] Create actions.rs with RedirectAction (ExecutionAction impl)
2. [ ] Implement TlsUpgradeAction (ExecutionAction impl)
3. [ ] Create HttpClientAction enum (combines all actions)
4. [ ] Implement ExecutionAction for HttpClientAction (delegate)
5. [ ] Create task.rs with HttpRequestState enum
6. [ ] Create HttpRequestTask struct with generic resolver
7. [ ] Implement TaskIterator for HttpRequestTask (state machine)
8. [ ] Create executor.rs with execute_task wrapper
9. [ ] Implement execute_single (valtron::single::spawn)
10. [ ] Implement execute_multi (valtron::multi::spawn, feature-gated)
11. [ ] Write comprehensive unit tests with WHY/WHAT docs

**Dependencies Met**:
- ✅ valtron-utilities (ExecutionAction types, unified executor)
- ✅ foundation (HttpClientError)
- ✅ connection (HttpClientConnection, ParsedUrl)
- ✅ request-response (PreparedRequest, ResponseIntro)

**Valtron Types to Use**:
- TaskIterator trait, TaskStatus enum, ExecutionAction trait
- NoSpawner/NoAction, DoNext wrapper
- single::spawn(), multi::spawn()
- spawn_builder with lift/schedule/broadcast

---

## Implementation Plan

**Phase 1: ExecutionAction Implementations** (Tasks 1-4)
- Create actions.rs
- Implement RedirectAction (spawns redirect requests)
- Implement TlsUpgradeAction (spawns TLS handshake)
- Create HttpClientAction enum combining both

**Phase 2: TaskIterator State Machine** (Tasks 5-7)
- Create task.rs
- Define HttpRequestState enum (Init → Connecting → ... → Done)
- Implement HttpRequestTask struct
- Implement TaskIterator trait with state transitions

**Phase 3: Executor Wrapper** (Tasks 8-10)
- Create executor.rs
- Implement execute_task with feature gates
- Implement execute_single (valtron::single)
- Implement execute_multi (valtron::multi, feature-gated)

**Phase 4: Testing** (Task 11)
- Comprehensive unit tests
- WHY/WHAT documentation per test
- Test all state transitions
- Test executor selection logic

---

## Files to Create/Modify

**New Files**:
- `backends/foundation_core/src/wire/simple_http/client/actions.rs`
- `backends/foundation_core/src/wire/simple_http/client/task.rs`
- `backends/foundation_core/src/wire/simple_http/client/executor.rs`

**Modified Files**:
- `backends/foundation_core/src/wire/simple_http/client/mod.rs` (add re-exports)

---

## Completed Features (5/13)

- ✅ valtron-utilities (33/33 tasks, 100%)
- ✅ tls-verification (48/48 tasks, 100%)
- ✅ foundation (9/9 tasks, 100%)
- ✅ connection (11/11 tasks, 100%)
- ✅ request-response (10/10 tasks, 100%)

## Remaining Features (8/13)

- 🔄 task-iterator (0/11 tasks) ← CURRENT (CRITICAL PATH)
- ⏳ compression (0/14 tasks)
- ⏳ proxy-support (0/13 tasks)
- 🔓 auth-helpers (0/13 tasks) - just unblocked
- 🔒 public-api (0/17 tasks) - needs task-iterator
- 🔒 cookie-jar (0/17 tasks) - needs public-api
- 🔒 middleware (0/13 tasks) - needs public-api
- 🔒 websocket (0/17 tasks) - needs connection + public-api

---

## Critical Notes for Implementation

**INTERNAL TYPES ONLY**:
- All types in this feature are pub(crate) or private
- Users should NOT see TaskIterator, TaskStatus, ExecutionAction
- Public API will wrap this in next feature

**NO ASYNC/AWAIT**:
- Use valtron TaskIterator patterns
- Iterator-based state machine
- NO tokio, NO async functions

**State Machine Pattern**:
- Non-blocking transitions
- Use TaskStatus::Spawn for child tasks
- Redirects and TLS upgrades spawn via ExecutionAction

**Feature Gates**:
- WASM always uses single executor
- Native uses single by default
- Native with "multi" feature uses multi executor

---

*Progress Report Updated: 2026-02-01*

*⚠️ Remember: This is EPHEMERAL. Permanent insights go to LEARNINGS.md*
