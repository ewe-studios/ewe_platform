# Decision 14: io_uring Reactor Backend (Linux) for Efficient FD Listening

> **Status:** foundation enhancement. Lands the real io_uring selector in
> `foundation_nativeapis`, which underpins the native parking model that Decision 00 (L2
> reactor seam) and Decision 13 E2 (WebSocket `Depends` read model) rely on. Linux-only,
> additive, with epoll fallback — nothing regresses on other platforms or without the
> `uring` feature.

## Context

`foundation_nativeapis` already owns a cross-platform readiness reactor (delivered by the
completed **spec 34 — native file watchers**, which scoped io_uring as *abstractions only*):

- `native::poll` — `Poll` / `Registry` / `Events` / `Token` / `Interest` / `SourceFd` /
  `Waker`, with concrete selectors in `poll/sys/unix/selector/`: **`epoll.rs`** (Linux) and
  **`kqueue.rs`** (macOS/BSD). Windows is stubbed.
- `native::fd` — `RegisteredFd<T: AsRawFd>` / `FdRegistration`, which **implement
  `foundation_core::valtron::EventReadiness`** — the bridge that lets a valtron task park via
  `TaskStatus::Depends(..)` on socket readiness (see Decisions 00 / 13).

**What's missing — io_uring is scaffolded but not implemented:**

- `io-uring = "0.7"` is an optional dep; a `uring` feature exists (`uring = ["dep:io-uring"]`)
  and `native-linux` enables it; `NativeAPI::IOUring` is the *advertised default* on Linux
  (`shared/api.rs`).
- But `try_build_api(NativeAPI::IOUring, …)` returns `Err(WatchError::UnsupportedPlatform)`
  in **both** the `uring` and `not(uring)` branches — so Linux "prefers" io_uring, fails, and
  silently falls back to inotify/poll. The `io-uring` crate is **never actually used** (only
  doc comments mention it). There is **no io_uring selector** under
  `poll/sys/unix/selector/`, so the fd-readiness reactor (`RegisteredFd`) is epoll-only.

**Why io_uring matters here.** The Connect server transports (HTTP/1.1, `http2/`, HTTP/3,
WebSocket Decision 13) will park many connections on the reactor. Two problems with the
current path at scale:

1. **Per-fd epoll instance.** `FdRegistration::new` creates its *own* `Poll` (a private epoll
   fd) per registered fd and does a **zero-timeout `epoll_wait` syscall per `is_ready()`
   check** (`native/fd/mod.rs`). At thousands of connections that is one epoll fd per
   connection plus a syscall per scheduler readiness poll — wasteful.
2. **Readiness vs completion.** epoll is *readiness*-based (tell me when readable, then I
   `read`); io_uring is *completion*-based (submit the read, get the bytes when done) —
   fewer syscalls, batched submission/completion, and a natural fit for the resumable
   decoder (Decision 12 §11) and the `Depends` seam.

## Decision

**Implement a real io_uring selector in `foundation_nativeapis` as an additional reactor
backend, selected via the existing `NativeAPI::IOUring` / `uring` feature, with epoll as the
automatic fallback. The `EventReadiness` seam and all transports are unchanged.**

### Scope

1. **`poll/sys/unix/selector/uring.rs`** — an io_uring-backed `Selector` sibling to
   `epoll.rs`/`kqueue.rs`, behind `#[cfg(all(target_os = "linux", feature = "uring"))]`,
   implementing the same internal selector interface (`register`/`reregister`/`deregister`/
   `poll(events, timeout)` → `Events`). Uses **multishot poll** (`IORING_OP_POLL_ADD` /
   poll-multishot) for readiness so it slots behind the existing `Poll`/`Registry` API with
   no change to callers; completion-mode read/write (`IORING_OP_RECV`/`SEND`) is a **second
   committed feature (14-F4)**, sequenced after the readiness selector's parity suite is
   green (OQ#14.2, resolved).
2. **Runtime selection + fallback.** `Poll::new()` selects the backend via the functional
   probe (OQ#14.3): `NativeAPI::Auto` walks the ladder uring-completion → uring-readiness →
   epoll and surfaces the choice; **explicit `NativeAPI::IOUring` is a requirement** — probe
   failure at the uring→epoll boundary is a hard error carrying the probe detail (no silent
   demotion; no-silent-defaults). Fix `shared/api.rs` so `NativeAPI::IOUring` actually
   builds the io_uring path instead of returning `UnsupportedPlatform`.
3. **Shared single reactor (fixes the per-fd-epoll scaling issue).** Introduce one
   process/pool-level reactor instance with a `Token → wake` map, so `RegisteredFd` /
   `FdRegistration` register into the **shared** selector instead of each creating a private
   `Poll`. This applies to *both* epoll and io_uring backends (epoll benefits immediately).
   `EventReadiness::is_ready` consults the shared reactor's last-drained readiness for the
   token rather than doing its own zero-timeout syscall.
4. **Unchanged seam.** `RegisteredFd<T>: EventReadiness` stays the public contract; the WS /
   HTTP server tasks still return `Depends(registered_fd)` (Decision 13 E2). io_uring is a
   pure backend swap underneath — no transport, codec, or handler code changes.

### Non-goals (this decision)

- Completion-based **file** I/O — network completion mode is committed as 14-F4 (OQ#14.2,
  resolved), but file I/O over io_uring stays out of scope; 14-F4 is not required to land
  efficient fd listening (14-F1..F3 stand alone).
- Non-Linux platforms — kqueue/IOCP remain their respective backends.
- Replacing epoll — epoll stays as the portable Linux fallback and the default when `uring`
  is disabled.

## Consequences

- **Decision 00 L2 is reconciled, not reinvented:** `foundation_nativeapis` is the platform
  reactor that already produces `EventReadiness`; this decision upgrades its Linux backend.
  No new `ReadinessSource`/`OnceLock` abstraction is needed — `EventReadiness` is the seam.
- **Decision 13 E2 gets a high-performance Linux backend for free** — the WS `Depends` read
  model runs on io_uring where available, epoll otherwise, with identical task code.
- **The shared-reactor change (item 3) is the bigger near-term win** than io_uring itself: it
  removes the per-fd epoll instance + per-check syscall for *all* native fd parking, on every
  Unix platform.
- Linux-only and feature-gated; other targets and `--no-default-features` builds are
  unaffected. `cargo check` on wasm/macos/windows unchanged.
- Depends on the kernel: multishot poll needs Linux ≥ 5.13; the probe-and-fallback keeps
  older kernels working on epoll.

## Decided Details

- **OQ#14.1 — resolved: one process-level shared reactor** (OnceLock singleton, mirroring
  the valtron pool) holding the selector + a concurrent `Token → wake` map; applies to both
  epoll and io_uring backends. Contention is acceptable by construction in readiness mode:
  multishot poll submits **once per fd** at registration, so the map is written rarely
  (accept/close) and read often (`is_ready` reads last-drained readiness — zero syscalls on
  the check path); io_uring's single-submitter SQ needs a lock only on that rare
  registration path. The drain strategy (dedicated drain thread parked in `poll(timeout)`
  pushing wakes, vs opportunistic draining by the first checker on cache miss) is a 14-F1
  implementation detail. **Committed re-eval trigger:** when completion mode lands (14-F4,
  OQ#14.2), per-op submissions make SQ contention real — revisit per-worker rings *then*;
  the `EventReadiness` seam is unchanged either way. Per-worker-now was rejected: it
  needs fd→worker placement rules, a home for non-worker registrations, N-way
  drain/observability, and makes valtron task-pinning a load-bearing invariant of the I/O
  layer — costs paid up front for a benefit that only materializes in completion mode.
  - **Re-evaluated 2026-07-10, on 14-F4 landing: the trigger did not fire. Keep the single
    shared reactor.** The premise — "per-op submissions" — is false for the implementation
    that shipped. Multishot `RECV` (`IORING_RECV_MULTISHOT`) arms an fd with **one** SQE for
    its entire lifetime; the kernel then delivers every subsequent message without any
    further submission. Submissions are therefore `O(registrations)`, exactly as in readiness
    mode, not `O(reads)`. Measured, not assumed: `submissions_are_per_registration_not_per_read`
    registers one socket (1 SQE) and drives 64 message round-trips, asserting the SQE count is
    unchanged. It is. The SQ lock is still taken only on accept/close, so it is not a
    contention point and per-worker rings buy nothing here. If that test ever fails the
    premise has returned and this question re-opens.
  - **Where the contention actually moved:** to `BufRing`'s tail mutex, taken once per
    `ProvidedBuf` drop — i.e. once per delivered message, from whichever thread finished with
    the bytes. That is a short critical section (two stores) with no syscall, and it is a
    *different* problem from SQ contention: it is fixed by making the tail a lock-free atomic
    CAS, not by sharding rings per worker. Left as-is until a profile says otherwise.
  - The one case that would restore the original premise is the documented
    **re-arm-per-recv fallback** for kernels with buffer rings but without `RECV_MULTISHOT`
    (5.19 ≤ kernel < 6.0). There, each delivery costs a fresh SQE. The functional probe
    already distinguishes this tier (`recv_multishot`), so if that fallback is ever
    implemented it should carry its own SQ-contention measurement.
- **OQ#14.2 — resolved: both modes, as two committed, sequenced features** (readiness first
  with cover tests, completion next — not "completion maybe later").
  - **14-F2 (readiness, kernel ≥ 5.13):** multishot `POLL_ADD` behind the existing internal
    `Selector` interface; `Poll`/`Registry` callers and the pull-model read path unchanged.
    Ships with a **uring↔epoll behavioral parity test suite** (same events for same stimuli,
    edge triggers, fd close mid-poll, drain ordering) — that suite later defines what F4's
    fallback must preserve. F4 does not start until it is green.
  - **14-F4 (completion, kernel ≥ 5.19 buffer rings / ≥ 6.0 multishot recv):** the kernel
    performs the read; the CQE carries the bytes (buffer id + len from a registered buffer
    ring) — zero `read()` syscalls on the hot path. Scope: a **`CompletionSource` seam in
    foundation_nativeapis next to `EventReadiness`** (`take_completions` + buffer `recycle`
    ownership protocol); transports opt their read path into inbox-pop; **per-worker rings
    re-evaluated here** (the OQ#14.1 trigger — per-op submission makes SQ contention real);
    kernel probe extended; kernels below the matrix fall back to F2 readiness mode.
  - **Seam analysis (why this stays out of valtron):** valtron's contract — task returns
    `Depends(source)`, reactor calls `wake(token)`, task is re-polled — is **byte-blind and
    identical in both modes**; it cannot tell a readiness bit from a completed recv. The
    entire delta lives in foundation_nativeapis (ring + buffer lifecycle) and the netio
    transport tasks (byte acquisition: `stream.read()` syscall vs popping a kernel-filled
    buffer). The Decision 12 §11 `IncrementalDecoder` is unchanged in both modes: its
    `step(&mut impl Read)` accepts a byte-slice cursor (`&[u8]: Read`) as happily as a
    socket, and its accumulating buffer already handles frames spanning two completion
    buffers.
- **OQ#14.3 — resolved: functional opcode probe as the only gate; explicit `IOUring` =
  hard requirement, `Auto` = surfaced best-available.**
  - **Kernel matrix (documentation, never the gate):** multishot poll ≥ 5.13 (14-F2);
    provided buffer rings ≥ 5.19 and multishot recv ≥ 6.0 (14-F4; recv-multishot absent →
    re-arm-per-recv variant).
  - **Probe (once, at shared-reactor init, cached in the OnceLock reactor):**
    (1) `io_uring_setup` on a small ring — catches `CONFIG_IO_URING=n` and
    `kernel.io_uring_disabled` (sysctl, 6.6+, common on hardened hosts) regardless of
    version; (2) `IORING_REGISTER_PROBE` → supported-opcode bitmap; (3) tier checks —
    readiness tier (`POLL_ADD` + `IORING_POLL_ADD_MULTI`), completion tier (`PBUF_RING`
    registration attempt, `RECV_MULTISHOT` flag). Version strings (`uname`) appear in log
    lines for humans only — distros backport (enterprise "5.14" ≫ mainline 5.14) and
    restrict, so version gates produce both false negatives and false positives.
  - **Selection ladder:** uring-completion → uring-readiness → epoll, each demotion logged
    with its concrete probe failure.
  - **Preference semantics (amends Scope §2):** `NativeAPI::Auto` / `native_default_api()`
    walks the ladder and surfaces the choice — a documented selection policy, not a silent
    default. **Explicit `NativeAPI::IOUring`: probe failure at the uring→epoll boundary is
    a hard error** carrying the probe detail (e.g. "io_uring unavailable:
    kernel.io_uring_disabled=2") — no silent demotion; the internal completion→readiness
    demotion on 5.13–5.18 kernels is fine (caller asked for uring, got uring). Explicit
    `NativeAPI::Epoll`: epoll, no probe. Rationale: an explicitly requested backend being
    silently swapped is the no-silent-defaults anti-pattern, and the ops failure mode —
    perf-critical deploy silently on epoll, discovered via latency graphs — is the worst
    one.
  - **Observability:** `reactor.backend() → {Epoll, UringReadiness, UringCompletion}` +
    `tracing::info!` at init (tier chosen, opcode-bitmap summary, demotion reasons).

## Features (generated from this decision)

| # | Feature | Depends on |
|---|---|---|
| 14-F1 | **Shared reactor + `Token→wake` map** — `RegisteredFd`/`FdRegistration` register into one shared selector instead of a private `Poll` each; `is_ready` reads drained readiness. Backend-agnostic (epoll today). | — |
| 14-F2 | **io_uring selector** `poll/sys/unix/selector/uring.rs` (multishot poll) behind `cfg(linux, feature="uring")`, plugged into `Poll`/`Registry`. | 14-F1 |
| 14-F3 | **Runtime selection + fallback** — fix `shared/api.rs`/`Poll::new` so `NativeAPI::IOUring` builds io_uring, gated by the functional opcode probe (OQ#14.3); `Auto` = surfaced ladder fallback, explicit `IOUring` = hard error on probe failure; `reactor.backend()` observability. | 14-F2 |
| 14-F4 | **Completion-mode read path** — `CompletionSource` seam in nativeapis (`take_completions` + buffer-ring `recycle`); `RECV`/`SEND` with registered buffer rings; transports opt read path into inbox-pop (decoder unchanged — `step(&mut &buf[..])`); per-worker rings re-eval (OQ#14.1 trigger); extended kernel probe (≥ 5.19/6.0), fallback to F2 readiness mode. Starts only after F2's parity suite is green. | 14-F2, 14-F3 |

## Success Criteria

- `NativeAPI::IOUring` on a supported kernel actually drives an io_uring selector (no
  `UnsupportedPlatform`); on an unsupported kernel it fails hard with the probe reason.
  `NativeAPI::Auto` on an old kernel selects epoll and surfaces the choice
  (`reactor.backend()` + init log).
- `RegisteredFd` parking uses **one shared reactor** — no private epoll per fd, no
  zero-timeout syscall per `is_ready`; verified by fd/syscall counts under N idle connections.
- A WS/HTTP server task parks via `Depends(RegisteredFd)` and is woken by the io_uring backend
  (Decision 13 E2 path) with identical task code to the epoll path.
- `cargo check -p foundation_nativeapis` passes with and without `--features uring`, and on
  wasm/macos/windows unchanged.

## Module References

- `backends/foundation_nativeapis/src/native/poll/sys/unix/selector/` — `epoll.rs`,
  `kqueue.rs` (+ new `uring.rs`)
- `backends/foundation_nativeapis/src/native/fd/mod.rs` — `RegisteredFd`, `FdRegistration`,
  `EventReadiness` impls (shared-reactor refactor)
- `backends/foundation_nativeapis/src/shared/api.rs` — `NativeAPI`, `native_default_api`,
  selection/fallback
- `backends/foundation_nativeapis/Cargo.toml` — `io-uring` dep, `uring` / `native-linux`
  features
- Reference specs: `specifications/completed/34-native-file-watchers` (reactor + fd mgmt
  origin), `specifications/completed/37-overlay-vfs` (nativeapis valtron integration patterns)
