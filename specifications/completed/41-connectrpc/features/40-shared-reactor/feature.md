---
feature: "Shared reactor + Token→wake map (D14 F1)"
description: "One process-level reactor instead of per-fd epoll instances; is_ready reads drained readiness"
status: "complete"
priority: "medium"
phase: 2
depends_on: ["10-reactor-parking"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 40-shared-reactor: Shared reactor + Token→wake map (D14 F1)

## Description

The bigger near-term win of Decision 14: kill the per-fd epoll instance and per-check syscall for ALL native fd parking, on every Unix platform, backend-agnostic.

## Normative sources (single source of truth — read before writing code)

- decisions/14-io-uring-reactor-backend.md — Scope #3 + Decided Details OQ#14.1 (normative)

## Scope

- OnceLock singleton reactor (selector + concurrent Token→wake map); RegisteredFd/FdRegistration register into it
- is_ready consults drained readiness — zero syscalls on the check path; drain strategy is an implementation detail

## Acceptance criteria

- N idle connections: no private epoll fd each, no per-check syscall (fd/syscall count test — D14 success criteria)
- The reactor's wake path fires end to end: a registered fd that becomes readable turns `is_ready()` true
- Readiness is edge-consumable: after a `WouldBlock` read clears it, `is_ready()` returns false until the next edge

## Reopened 2026-07-10 — completion was premature; re-closed the same day

Marked complete without a working wake path. Audit findings, all reproduced by test, all now
fixed and covered by `tests/reactor_shared_selector_tests.rs` (5 tests) and the repaired
`tests/reactor_parking_tests.rs` (2 tests), green on **both** the epoll and io_uring backends:

1. **Dead wake path.** `Reactor::get()` built one `Poll` for registrations and a *second*
   independent `Poll::new()` for the drain thread. Registrations landed in selector A; the
   drain thread polled selector B, which had no fds. `is_ready()` read a cache nothing ever
   wrote — permanently `false`, so every task parked via `Depends(RegisteredFd)` parked
   forever. Silent: nothing errored. Pinned by `tests/reactor_shared_selector_tests.rs`.
2. **No readiness clear protocol.** `entry.ready` was `union`-only, never cleared, and
   `ReadyGuard::clear_ready()` assigned `Ready::EMPTY` to the guard's own local copy — dropped
   with the guard, never reaching `FdRegistration` or the reactor cache. Once (1) is fixed a
   registration would go permanently-ready and consumers would spin instead of parking.
   Under edge-triggered epoll the cached bit must be cleared on `WouldBlock` to re-arm.
3. **Readiness bits discarded.** `FdRegistration::query_readiness` returned a hardcoded
   `READABLE|WRITABLE` on any reactor readiness, so `is_read_closed()` / `is_error()` could
   never be observed through the shared-reactor path.
4. **Proof never compiled.** `tests/reactor_parking_tests.rs` — the file whose header calls
   itself the end-to-end proof of native fd parking — failed to build (8 errors: `EventReadiness`
   gained `Send + Sync` under `multi`; `RegisteredFd::new` signature drift). It was the only
   broken test target in the crate, so `cargo test -p foundation_nativeapis` was red.

### Fixes

- `Reactor` holds an `Arc<Poll>` and hands the drain thread *that* clone. The second
  `Poll::new()` is gone; a comment marks why a second selector is fatal.
- `Entry.ready` is an `AtomicU8`: the drain thread `fetch_or`s bits under a read lock,
  consumers `fetch_and` them out. The set/clear race resolves toward a spurious wake, which
  consumers already tolerate; a real edge can never be lost.
- `Reactor::clear` / `FdRegistration::clear_readiness` / `ReadyGuard::clear_ready` now form a
  real chain to the cache, so `try_io`'s `WouldBlock` branch re-arms the edge as documented.
- `Reactor::readiness()` returns the latched `Ready`; `query_readiness` forwards it verbatim.
- `Ready` gained `bits`/`from_bits`/`ALL` + `Display`; `FdRegistration::deregister` now
  routes through the reactor so cache entries are dropped with the registration.
- `Reactor::backend()` reports the selector in use (`Backend` enum) and logs it at init.
