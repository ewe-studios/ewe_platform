---
feature: "Signal Module"
description: "Cross-platform signal handling — SignalKind, SignalEvent, SignalBus, SignalTask with eventfd/kqueue/SetConsoleCtrlHandler backends, behind default-enabled signal feature flag"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: "2026-06-15"
---

# Feature: Signal Module

## Task Breakdown

1. [ ] Add `signal` feature to `Cargo.toml` (default), gate on `poll` feature
2. [ ] Create `signal/mod.rs` — `SignalKind`, `SignalEvent`, `SignalError`, `SignalBus`
3. [ ] Implement `SignalTask` — valtron `TaskIterator` with `Depends` parking
4. [ ] Implement Linux backend — `eventfd` + `sigaction` + `epoll`
5. [ ] Implement macOS backend — `kqueue EVFILT_SIGNAL`
6. [ ] Implement Windows backend — `SetConsoleCtrlHandler` + `WaitForSingleObject`
7. [ ] Re-export from crate root: `pub mod signal` (cfg-gated)
8. [ ] Write tests: self-SIGUSR1 delivery (Unix only)
9. [ ] Verify `cargo check` on all platforms

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_nativeapis/Cargo.toml` | Edit — add `signal` feature |
| `backends/foundation_nativeapis/src/signal/mod.rs` | Create |
| `backends/foundation_nativeapis/src/signal/linux.rs` | Create |
| `backends/foundation_nativeapis/src/signal/macos.rs` | Create |
| `backends/foundation_nativeapis/src/signal/windows.rs` | Create |
| `backends/foundation_nativeapis/src/lib.rs` | Edit — add `pub mod signal` |

---

_Created: 2026-06-15_
