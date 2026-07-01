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
**Has Features:** Yes (18 features, 17 complete, 1 pending)

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
| [reader-eof-handling](./02-build-http-client/features/reader-eof-handling/) | **CRITICAL**: Fix reader infinite loop on EOF Ok(0) | 5 | None |

**Total Tasks:** 148

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

### [09: Valtron StreamIterator Migration](./09-valtron-streamiterator/)
**Status:** ⏳ Pending
**Description:** Migrate Stream/StreamIterator from mpp to valtron, add ConcurrentQueueStreamIterator with configurable max_turns polling optimization.
**Has Features:** Yes (3 features)

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| [01-stream-migration](./09-valtron-streamiterator/features/01-stream-migration/) | Move `Stream` enum and `StreamIterator` trait from `synca::mpp` to `valtron::streams` | None |
| [02-concurrent-queue-iterator](./09-valtron-streamiterator/features/02-concurrent-queue-iterator/) | Implement `ConcurrentQueueStreamIterator` with `max_turns` polling optimization | #1 |
| [03-import-updates](./09-valtron-streamiterator/features/03-import-updates/) | Update all imports across codebase, re-export for backward compatibility | #1, #2 |

**Total Tasks:** TBD

---

### [12: Background Job Registry](./12-background-job-registry/)
**Status:** ✅ Completed
**Description:** Add a BackgroundJobRegistry to valtron that owns a fixed pool of background worker threads for executing blocking closures, replacing ad-hoc thread spawning in ThreadedIterFuture and exposing a unified `run_background_job` API.
**Has Features:** Yes (6 features, 100% complete)
**Builds On:** [09-multi-threaded-executor-improvements](./09-multi-threaded-executor-improvements/)

| Feature | Description | Tasks |
|---------|-------------|-------|
| 01: Core | BackgroundJobRegistry struct, worker loop, panic protection | 8 ✅ |
| 02: Pool Integration | Thread allocation formula, multi/mod.rs integration | 7 ✅ |
| 03: Single/Unified API | run_background_job in single + unified modules | 4 ✅ |
| 04: ThreadedIterFuture Migration | Replace std::thread::spawn with run_background_job | 4 ✅ |
| 05: Feature Gating | Feature-gated ThreadedIterFuture implementations (multi, std, no_std) | 11 ✅ |
| 06: Unified run_future_iter API | Consolidate into single unified function | 10 ✅ |

**Total Tasks:** 44 (all completed)

---

### [17: Foundation JSON Schema](./17-foundation-jsonschema/)
**Status:** ⏳ Pending
**Description:** Self-contained JSON Schema validation library for ewe_platform supporting Drafts 4/6/7/2019-09/2020-12 with no_std compatibility, no reqwest/tokio/wasm-bindgen dependencies, and trait-based external reference resolution via `JsonResolver`.
**Has Features:** Yes (11 features)

| # | Feature | Description | Dependencies | Tasks |
|---|---------|-------------|--------------|-------|
| 0 | [core-types](./17-foundation-jsonschema/features/00-core-types/) | JsonType, paths, Draft enum, JsonResolver trait | None | 28 |
| 1 | [referencing](./17-foundation-jsonschema/features/01-referencing/) | URI resolution, Registry, Resolver, anchors, vocabulary | 0 | 42 |
| 2 | [keywords-validators](./17-foundation-jsonschema/features/02-keywords-validators/) | 35+ keyword validator implementations | 0, 1 | 52 |
| 3 | [compiler](./17-foundation-jsonschema/features/03-compiler/) | Schema compilation pipeline | 0, 1, 2 | 24 |
| 4 | [validation-engine](./17-foundation-jsonschema/features/04-validation-engine/) | Runtime validation, cycle detection, evaluation output | 0–3 | 22 |
| 5 | [error-reporting](./17-foundation-jsonschema/features/05-error-reporting/) | Structured error types, error iterator, path tracking | 0 | 20 |
| 6 | [draft-support](./17-foundation-jsonschema/features/06-draft-support/) | Per-draft modules, meta-schema validation | 0, 1, 2 | 18 |
| 7 | [format-validation](./17-foundation-jsonschema/features/07-format-validation/) | 19 built-in format validators (email, uri, date-time, etc.) | 0, 2 | 22 |
| 8 | [custom-extensions](./17-foundation-jsonschema/features/08-custom-extensions/) | Custom keyword factory, custom format validators | 0, 2, 3, 4 | 12 |
| 9 | [test-suite](./17-foundation-jsonschema/features/09-test-suite/) | Official JSON Schema Test Suite (~7200 tests), integration tests | All | 30 |
| 10 | [fuzz-targets](./17-foundation-jsonschema/features/10-fuzz-targets/) | Fuzz targets for compilation, validation, referencing | All | 10 |

**Total Tasks:** 280

---

### [23: Valtron Executor Deep Dive](./23-valtron-executor-deep-dive/)
**Status:** ✅ Completed
**Description:** Comprehensive analysis of valtron executor architecture, identifying core behaviors, logic patterns, footguns, and improvement opportunities. Fixes CondVar/park_timeout mismatch causing ~18s test delays.
**Has Features:** Yes (4 features)
**Builds On:** [09-multi-threaded-executor-improvements](./09-multi-threaded-executor-improvements/)
**Related To:** [03-wasm-friendly-sync-primitives](./03-wasm-friendly-sync-primitives/), [04-condvar-primitives](./04-condvar-primitives/), [12-background-job-registry](./12-background-job-registry/)

| Feature | Description | Tasks | Dependencies |
|---------|-------------|-------|--------------|
| [00-core-architecture-analysis](./23-valtron-executor-deep-dive/features/00-core-architecture-analysis/) | Deep dive into executor components, state machines, TaskStatus variants | 12 | None |
| [01-thread-yielder-improvements](./23-valtron-executor-deep-dive/features/01-thread-yielder-improvements/) | Fix CondVar/park_timeout mismatch | 8 | #00 |
| [02-wasm-compatibility](./23-valtron-executor-deep-dive/features/02-wasm-compatibility/) | no_std-compatible yielding strategy | 6 | #00, #01 |
| [03-shutdown-mechanism-fixes](./23-valtron-executor-deep-dive/features/03-shutdown-mechanism-fixes/) | Proper sleeper notification | 7 | #00, #01 |

**Total Tasks:** 33
**Key Issues:**
- CondVar/park_timeout mismatch (CRITICAL) - causes ~18s test delays
- NoThreadController does nothing (HIGH) - 100% CPU busy-waiting
- Sleeping tasks not notified on shutdown (HIGH) - uninterruptible delays
- drivers.rs busy-waiting (MEDIUM) - inefficient CPU usage

---

### [25: Valtron Quality Improvements](./25-valtron-quality-improvements/)
**Status:** ⏳ Pending
**Description:** Address architectural issues, semantic bugs, and quality improvements in valtron module identified during deep-dive review. Covers execution-model correctness, iterator semantics, error propagation, and code quality.
**Has Features:** Yes (9 features)
**Builds On:** [23-valtron-executor-deep-dive](./23-valtron-executor-deep-dive/)

| Feature | Priority | Description | Tasks |
|---------|----------|-------------|-------|
| [00-pending-none-livelock](./25-valtron-quality-improvements/features/00-pending-none-livelock/) | CRITICAL | Fix unbounded CPU spin on Pending(None) | 9 |
| [01-sleeper-lifecycle-safety](./25-valtron-quality-improvements/features/01-sleeper-lifecycle-safety/) | CRITICAL | Fix stale sleeper panic on task combination | 6 |
| [02-linked-task-state-propagation](./25-valtron-quality-improvements/features/02-linked-task-state-propagation/) | HIGH | Fix DualSequence discarding parent State | 5 |
| [03-global-queue-fairness](./25-valtron-quality-improvements/features/03-global-queue-fairness/) | HIGH | Fairness for global queue pickup | 4 |
| [04-notification-based-waiting](./25-valtron-quality-improvements/features/04-notification-based-waiting/) | HIGH | Replace spin+sleep with CondVar notification | 6 |
| [05-iterator-semantics](./25-valtron-quality-improvements/features/05-iterator-semantics/) | HIGH | TaskIterator recursion trap, TransformIterator, swap_remove | 6 |
| [06-error-handling-api](./25-valtron-quality-improvements/features/06-error-handling-api/) | MEDIUM | Error swallowing, executor panics, Drain bounds | 5 |
| [07-channel-backpressure](./25-valtron-quality-improvements/features/07-channel-backpressure/) | MEDIUM | Bounded queues, EntryList slot reuse | 4 |
| [08-code-quality](./25-valtron-quality-improvements/features/08-code-quality/) | LOW | WASM fix, BranchPath naming, alias cleanup | 3 |

**Total Tasks:** 48

---

### [51: llama.cpp Multi-Token Prediction (MTP)](./51-llama-mtp-speculative/)
**Status:** 🔄 In Progress
**Description:** Opt-in, capability-gated MTP / speculative decoding for the llama.cpp provider, surfaced through the foundation_ai harness presets for the models that support it (GLM 5.2, Qwen 3.6, Gemma 4). Records the harness module, llama.cpp b9850 upgrade, and Jinja/minja chat-template shim as background.
**Has Features:** No (tasks tracked in requirements.md)
**Builds On:** [36-agentic-api](./completed/36-agentic-api/)

---

## Status Dashboard

### Summary
- **Total Specifications:** 10
- **Completed:** 5 (50%)
- **In Progress:** 1 (10%)
- **Pending:** 4 (40%)

### Completed ✅
- 01: Fix Rust Lints, Checks, and Styling
- 03: WASM-Friendly Sync Primitives
- 04: CondVar Primitives
- 12: Background Job Registry
- 23: Valtron Executor Deep Dive

### In Progress 🔄
- 02: Build HTTP Client

### Pending ⏳
- 07: TCP-Resilient Batch Readers
- 09: Valtron StreamIterator Migration
- 17: Foundation JSON Schema
- 25: Valtron Quality Improvements

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
*Last updated: 2026-05-12*
