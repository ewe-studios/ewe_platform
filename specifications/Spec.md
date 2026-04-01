# Project Specifications

## Overview
This directory contains all project specifications and requirements. Each specification represents a significant feature, enhancement, or change to the project.

## How Specifications Work

1. **Requirements-First**: Before work begins, main agent discusses requirements with user
2. **Documentation**: Requirements and tasks are documented in numbered specification directories
3. **User Approval**: User must explicitly approve and request implementation
4. **Agent Reading**: Agents MUST read requirements.md and relevant feature.md files
5. **Status Verification**: Agents MUST verify completion status by searching the codebase
6. **Task Updates**: Agents MUST update task tracking in requirements.md or feature.md files
7. **Status Accuracy**: Agents MUST ensure status reflects actual implementation

**Task Tracking:**
- **Simple specs** (has_features: false): Tasks tracked in requirements.md
- **Feature-based specs** (has_features: true): Tasks tracked in individual feature.md files

## All Specifications

### [01: Fix Rust Lints, Checks, and Styling](./01-fix-rust-lints-checks-styling/)
**Status:** ✅ Completed
**Description:** Systematic resolution of all pending Rust lints, checks, and styling mistakes across the ewe_platform codebase.

---

### [02: Build HTTP Client](./02-build-http-client/)
**Status:** 🔄 In Progress
**Description:** Create an HTTP 1.1 client using existing simple_http module structures with iterator-based patterns and valtron executors.
**Has Features:** Yes (13 features)

| Feature | Description | Tasks | Dependencies |
|---------|-------------|-------|--------------|
| [valtron-utilities](./02-build-http-client/features/valtron-utilities/) | ExecutionAction types, unified executor, Future adapter | 24 | None |
| [tls-verification](./02-build-http-client/features/tls-verification/) | Verify/fix TLS backends | 8 | valtron-utilities |
| [foundation](./02-build-http-client/features/foundation/) | Error types and DNS resolution | 7 | tls-verification |
| [compression](./02-build-http-client/features/compression/) | gzip, deflate, brotli support | 9 | foundation |
| [connection](./02-build-http-client/features/connection/) | URL parsing, TCP, TLS | 4 | foundation |
| [proxy-support](./02-build-http-client/features/proxy-support/) | HTTP/HTTPS/SOCKS5 proxy | 14 | connection |
| [request-response](./02-build-http-client/features/request-response/) | Request builder, response types | 4 | connection |
| [auth-helpers](./02-build-http-client/features/auth-helpers/) | Basic, Bearer, Digest auth | 10 | request-response |
| [task-iterator](./02-build-http-client/features/task-iterator/) | TaskIterator, executors | 8 | request-response, valtron-utilities |
| [public-api](./02-build-http-client/features/public-api/) | User-facing API, integration | 6 | task-iterator |
| [cookie-jar](./02-build-http-client/features/cookie-jar/) | Automatic cookie handling | 15 | public-api |
| [middleware](./02-build-http-client/features/middleware/) | Request/response interceptors | 14 | public-api |
| [websocket](./02-build-http-client/features/websocket/) | WebSocket client and server | 20 | connection, public-api |

**Total Tasks:** 143

---

### [03: WASM-Friendly Sync Primitives](./03-wasm-friendly-sync-primitives/)
**Status:** ✅ Completed
**Description:** Implement no_std-compatible spin-based synchronization primitives (SpinMutex, SpinRwLock, Once) for foundation_nostd with WASM optimization.
**Has Features:** No
**Has Fundamentals:** Yes (9 fundamental documents)

**Key Components:**
- `SpinMutex<T>` - Spin-based mutex with poisoning
- `SpinRwLock<T>` - Writer-preferring read-write lock with poisoning
- `ReaderSpinRwLock<T>` - Reader-preferring variant
- `Once` - One-time initialization primitive
- WASM single-threaded optimization (no-op locks)
- 16 primitives total with comprehensive documentation

**Total Tasks:** 48 (100% complete)
**Verification:** All tests passed, 0 clippy warnings, production ready

---

### [04: CondVar Primitives](./04-condvar-primitives/)
**Status:** ✅ Completed
**Description:** Implement CondVar (Condition Variable) primitives in foundation_nostd for no_std and WASM contexts with full std::sync::Condvar API compatibility.
**Has Features:** No
**Has Fundamentals:** Yes (7 fundamental documents)
**Builds On:** [03-wasm-friendly-sync-primitives](./03-wasm-friendly-sync-primitives/)

**Key Components:**
- `CondVar` - Full std::sync::Condvar compatibility with poisoning
- `CondVarNonPoisoning` - Simplified variant without poisoning overhead
- `RwLockCondVar` - Condition variable for read-write locks
- Integration with Mutex and RwLock from spec 03
- WASM optimization with single-threaded detection
- Complete wait/notify API (wait, wait_while, wait_timeout, notify_one, notify_all)
- Bit-masking for compact state management

**Total Tasks:** 209 (90.9% complete - 190 tasks)
**Testing:** 227 tests passing (190 unit + 14 integration + 23 WASM)
**Verification:** All checks passed - zero clippy warnings, WASM verified
**Infrastructure:** Root Makefile with 40+ commands, foundation_testing crate created

---

### [07: TCP-Resilient Batch Readers](./07-tcp-resilient-batch-readers/)
**Status:** ⏳ Pending
**Description:** TCP-resilient batch readers that use `read()` instead of `read_exact()` to correctly handle WouldBlock/TimedOut on TCP streams.
**Has Features:** No
**Related:** [02-build-http-client](./02-build-http-client/)

**Key Components:**
- `BatchReader<R: Read>` - Iterator yielding byte batches or retry signals
- `FullBodyReader<R: Read>` - Known-size body reader with retry resilience
- Updated `SimpleHttpBody` - Threshold-based reader strategy selection

**Total Tasks:** 17

---

### [12: Background Job Registry](./12-background-job-registry/)
**Status:** ⏳ Pending
**Description:** Add a BackgroundJobRegistry to valtron that owns a fixed pool of background worker threads for executing blocking closures, replacing ad-hoc thread spawning in ThreadedIterFuture and exposing a unified `run_background_job` API.
**Has Features:** Yes (6 features)
**Builds On:** [09-multi-threaded-executor-improvements](./09-multi-threaded-executor-improvements/)

| Feature | Description | Tasks |
|---------|-------------|-------|
| 01: Core | BackgroundJobRegistry struct, worker loop, panic protection | 8 |
| 02: Pool Integration | Thread allocation formula, multi/mod.rs integration | 7 |
| 03: Single/Unified API | run_background_job in single + unified modules | 4 |
| 04: ThreadedIterFuture Migration | Replace std::thread::spawn with run_background_job | 4 |
| 05: Feature Gating | Feature-gated ThreadedIterFuture implementations (multi, std, no_std) | 11 |
| 06: Unified run_future_iter API | Consolidate into single unified function | 10 |

**Total Tasks:** 44

---

## Status Dashboard

### Summary
- **Total Specifications:** 6
- **Completed:** 3 (50%)
- **In Progress:** 1 (17%)
- **Pending:** 2 (33%)

### Completed ✅
- 01: Fix Rust Lints, Checks, and Styling
- 03: WASM-Friendly Sync Primitives
- 04: CondVar Primitives

### In Progress 🔄
- 02: Build HTTP Client

### Pending ⏳
- 07: TCP-Resilient Batch Readers
- 12: Background Job Registry

## Specification Guidelines

### For Agents
When working with specifications:
1. **Read main files first**: requirements.md (always contains overview and context)
2. **Check for features/**: If present, read relevant feature.md files for detailed requirements
3. **Check for templates/**: Read any templates referenced in requirements or features
4. **Verify before assuming**: Search the codebase to confirm task status
5. **Update as you go**: Mark tasks complete only when truly done
6. **Keep counts accurate**: Update task tracking in requirements.md or feature.md frontmatter
7. **Commit regularly**: Follow git workflow rules

**Task Tracking Pattern:**
- **has_features: false** → Tasks in requirements.md
- **has_features: true** → Tasks in feature.md files (one per feature)

### For Users
This dashboard provides:
- **Quick overview**: See all specifications at a glance
- **Status tracking**: Monitor progress on each specification
- **Navigation**: Links to detailed requirements and tasks
- **Transparency**: Clear view of what's done, in progress, and pending
- **Feature breakdown**: Understanding of complex specification structure

---
*Last updated: 2026-03-14*
