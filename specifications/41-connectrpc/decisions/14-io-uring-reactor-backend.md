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
   no change to callers; completion-mode read/write (`IORING_OP_RECV`/`SEND`) is a later,
   optional optimization (OQ#14.2).
2. **Runtime selection + fallback.** `Poll::new()` picks io_uring when `NativeAPI::IOUring` is
   preferred *and* the running kernel supports the needed ops (probe at init); otherwise falls
   back to epoll. Fix `shared/api.rs` so `NativeAPI::IOUring` actually builds the io_uring
   path instead of returning `UnsupportedPlatform`. Kernel too old / feature off →
   transparent epoll fallback (never an error to the caller).
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

- Full completion-based file I/O / fixed buffers / registered fds — listed as future
  optimizations (OQ#14.2), not required to land efficient fd listening.
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

## Open Questions

- **OQ#14.1** — reactor ownership: one global reactor (OnceLock singleton, like the valtron
  pool) vs one per worker thread. *Tentative:* one shared reactor + `Token→wake`, revisit if
  contention shows up.
- **OQ#14.2** — go completion-mode (`RECV`/`SEND`, registered buffers) for the hot read path,
  or stay readiness-mode (multishot poll) and keep the existing `Read`/`Write` + resumable
  decoder? *Tentative:* readiness-mode first (smallest change, keeps Decision 12 §11
  decoder); completion-mode as a later optimization.
- **OQ#14.3** — minimum kernel / probe matrix and how `NativeAPI` preference interacts with a
  runtime probe failure (surface which backend was chosen for observability).

## Features (generated from this decision)

| # | Feature | Depends on |
|---|---|---|
| 14-F1 | **Shared reactor + `Token→wake` map** — `RegisteredFd`/`FdRegistration` register into one shared selector instead of a private `Poll` each; `is_ready` reads drained readiness. Backend-agnostic (epoll today). | — |
| 14-F2 | **io_uring selector** `poll/sys/unix/selector/uring.rs` (multishot poll) behind `cfg(linux, feature="uring")`, plugged into `Poll`/`Registry`. | 14-F1 |
| 14-F3 | **Runtime selection + fallback** — fix `shared/api.rs`/`Poll::new` so `NativeAPI::IOUring` builds io_uring with kernel probe; transparent epoll fallback; observability of chosen backend. | 14-F2 |

## Success Criteria

- `NativeAPI::IOUring` on a supported kernel actually drives an io_uring selector (no
  `UnsupportedPlatform`); on an old kernel it transparently falls back to epoll.
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
