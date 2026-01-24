# Project Specifications

## Overview
This directory contains all project specifications and requirements. Each specification represents a significant feature, enhancement, or change to the project.

## How Specifications Work

1. **Requirements-First**: Before work begins, main agent discusses requirements with user
2. **Documentation**: Requirements and tasks are documented in numbered specification directories
3. **User Approval**: User must explicitly approve and request implementation
4. **Agent Reading**: Agents MUST read requirements.md, tasks.md, and relevant feature files
5. **Status Verification**: Agents MUST verify completion status by searching the codebase
6. **Task Updates**: Agents MUST update tasks.md as work progresses
7. **Status Accuracy**: Agents MUST ensure status reflects actual implementation

## All Specifications

### [01: Fix Rust Lints, Checks, and Styling](./01-fix-rust-lints-checks-styling/)
**Status:** ✅ Completed
**Description:** Systematic resolution of all pending Rust lints, checks, and styling mistakes across the ewe_platform codebase.

---

### [02: Build HTTP Client](./02-build-http-client/)
**Status:** ⏳ Pending
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

## Status Dashboard

### Summary
- **Total Specifications:** 4
- **Completed:** 3 (75%)
- **In Progress:** 0 (0%)
- **Pending:** 1 (25%)

### Completed ✅
- 01: Fix Rust Lints, Checks, and Styling
- 03: WASM-Friendly Sync Primitives
- 04: CondVar Primitives

### In Progress 🔄
- None currently

### Pending ⏳
- 02: Build HTTP Client

## Specification Guidelines

### For Agents
When working with specifications:
1. **Read main files first**: requirements.md AND tasks.md
2. **Check for features/**: If present, read relevant feature.md and tasks.md
3. **Check for templates/**: Read any templates referenced in requirements
4. **Verify before assuming**: Search the codebase to confirm task status
5. **Update as you go**: Mark tasks complete only when truly done
6. **Keep counts accurate**: Update frontmatter in tasks.md files
7. **Commit regularly**: Follow git workflow rules

### For Users
This dashboard provides:
- **Quick overview**: See all specifications at a glance
- **Status tracking**: Monitor progress on each specification
- **Navigation**: Links to detailed requirements and tasks
- **Transparency**: Clear view of what's done, in progress, and pending
- **Feature breakdown**: Understanding of complex specification structure

---
*Last updated: 2026-01-24*
