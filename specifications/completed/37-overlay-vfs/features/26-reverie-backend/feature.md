---
feature_name: "Reverie Ptrace Backend"
description: "Alternative ptrace backend using Meta's reverie framework for syscall interception. Higher-level API than raw nix ptrace, built-in child tracing for fork/clone. Deferred — reverie is not on crates.io and is experimental."
status: "deferred"
priority: "low"
phase: 4
created: 2026-06-09
updated: 2026-06-09
dependencies:
  - "09-ptrace-interceptor"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# Feature 26: Reverie Ptrace Backend

## Overview

Alternative backend for the ptrace interceptor (feature 09) using Meta's [reverie](https://github.com/facebookexperimental/reverie) framework. Reverie provides a higher-level `Tool` trait for syscall interception with built-in child process tracing.

### Why Deferred

- `reverie` is not published to crates.io — only available as a GitHub dependency
- Experimental status at Meta — API may change
- Requires specific Linux kernel version support
- The nix backend (feature 09) is fully functional and covers all use cases

### Tasks

- [ ] Implement `ReverieInterceptor` struct implementing `SyscallInterceptor`
- [ ] Implement Reverie `Tool` trait with syscall entry/exit handlers
- [ ] Dispatch intercepted syscalls to shared syscall handler
- [ ] Handle fork/clone via Reverie's built-in child tracing

## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

_Created: 2026-06-09 | Updated: 2026-06-09_
