---
feature: "Runtime backend selection + functional probe (D14 F3)"
description: "NativeAPI::IOUring actually builds io_uring; Auto walks the ladder; explicit request fails hard on probe failure"
status: "complete"
priority: "medium"
phase: 2
depends_on: ["41-uring-selector"]
estimated_effort: "small"
created: 2026-07-03
---
# Feature 42-uring-selection-probe: Runtime backend selection + functional probe (D14 F3)

## Description

Fix the current lie (IOUring advertised, UnsupportedPlatform returned): probe-gated selection with the no-silent-defaults rule for explicit backend requests.

## Normative sources (single source of truth — read before writing code)

- decisions/14-io-uring-reactor-backend.md — Scope #2 + Decided Details OQ#14.3 (normative probe + semantics)

## Scope

- Functional opcode probe (setup → REGISTER_PROBE → tier checks) — never version-string gating
- Auto: completion → readiness → epoll, surfaced; explicit IOUring: hard error with probe detail (no-silent-defaults)
- reactor.backend() observability + init tracing

## Acceptance criteria

- Explicit IOUring on an unsupported host errors with the probe reason; Auto on old kernels lands on epoll and says so

## Reopened 2026-07-10 — the ladder was a `#[cfg]`; re-closed the same day

### What was wrong

Backend choice is made at **compile time**, not by a probe:

```rust
// src/native/poll/sys/unix/mod.rs:3-13
#[cfg(all(target_os = "linux", not(feature = "uring")))]  pub mod selector { pub use epoll::…; }
#[cfg(all(target_os = "linux", feature = "uring"))]       pub mod selector { pub use uring::…; }
```

Enabling `uring` compiles epoll *out* of the build, so there is no epoll to fall back to at
runtime. Since `native-linux = ["native", "watcher-linux", "uring"]`, that is the shipping
Linux configuration. Consequences against D14 OQ#14.3:

- **No functional opcode probe.** The only "probe" is `IoUring::new()` succeeding inside the
  `Selector` constructor. `IORING_REGISTER_PROBE` is never called; no opcode bitmap; no tier
  checks (`POLL_ADD` + `IORING_POLL_ADD_MULTI`, `PBUF_RING`, `RECV_MULTISHOT`).
- **No selection ladder.** uring-completion → uring-readiness → epoll does not exist.
- **No `Auto` semantics and no hard-error semantics.** `Auto` cannot surface a choice it never
  makes, and an explicit `NativeAPI::IOUring` cannot fail hard with probe detail on a host
  where `CONFIG_IO_URING=n` or `kernel.io_uring_disabled=2` — the crate simply fails to build
  a selector with no epoll alternative compiled in.
- **No `reactor.backend()`** observability accessor and no init `tracing::info!`.

### Also found: the Linux default file watcher was never inotify

`native_default_api()` returned `NativeAPI::IOUring` on Linux, and `try_build_api`'s `IOUring`
arm constructed a uring `Selector`, threw it away, and returned `build_poll_watcher()` — the
stdlib metadata-polling watcher. So `WatcherBuilder::default()` on Linux silently produced a
`PollWatcher`, never an `InotifyWatcher`, which is exactly the silent fallback D14's Context
section complains about.

### Fixes

- **`native::poll::probe`** — the functional probe. `io_uring_setup` (catches
  `CONFIG_IO_URING=n` and `kernel.io_uring_disabled`) → `IORING_REGISTER_PROBE` (opcode bitmap)
  → functional checks for the two capabilities the bitmap *cannot* express, because they are
  flags on an opcode rather than opcodes: `IORING_POLL_ADD_MULTI` (armed on an already-readable
  eventfd) and `IORING_RECV_MULTISHOT` (submitted on a socketpair; `-EINVAL`/`-EOPNOTSUPP`
  means absent, `-ENOBUFS` or no completion means present), plus a real `PBUF_RING`
  register/unregister attempt. No version string is ever branched on.
- **`native::poll::backend`** — `Backend`, `BackendPreference`, `SelectionError`, and the
  ladder. `select()` runs the probe; `resolve()` is the pure decision function split out so the
  demotion and hard-error branches are testable with a synthetic `ProbeError` (a healthy kernel
  never takes them). `Auto` demotes to epoll and logs why; explicit `Uring` returns a
  `SelectionError` carrying the probe detail; explicit `Epoll` skips the probe.
- **`sys::unix::selector::dispatch::Selector`** — a runtime enum over epoll and uring, so both
  are in the binary and the fallback is reachable. `Poll::with_preference` and `Poll::backend()`
  expose it; `Reactor::backend()` now reports the selector actually in use rather than a
  compile-time constant.
- **`COMPLETION_IMPLEMENTED`** is `false` until F43. The probe reports the completion tier when
  the kernel has it (this box: all four capabilities true on 7.0), but the ladder still selects
  readiness rather than naming a backend it cannot construct. F43 flips the constant.
- **`shared/api.rs`** — `NativeAPI::Auto` added and made the default. A named reactor backend is
  validated through `backend::select` before any watcher is built, so `IOUring` on a host
  without io_uring fails with the probe reason instead of quietly yielding a `PollWatcher`. The
  watcher itself is inotify for every reactor backend; io_uring does not watch files.

### Suite

`tests/uring_probe_selection_tests.rs` — 13 tests under `--features uring`, 6 without.
Covers probe coherence (completion tier implies readiness tier), `Auto` never failing on Linux,
`Poll::new()` being the `Auto` ladder, `Auto` demoting to epoll on probe failure, explicit
`Uring` hard-erroring with the detail preserved, completion→readiness demotion staying on
io_uring, the ladder refusing to select completion before F43, and `NativeAPI::Auto` being the
default. Without the `uring` feature, an explicit `Uring` request is `Unsupported` rather than a
silent epoll.
