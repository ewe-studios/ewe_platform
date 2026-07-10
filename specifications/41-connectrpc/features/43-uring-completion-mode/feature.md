---
feature: "io_uring completion-mode read path (D14 F4)"
description: "CompletionSource seam + buffer rings: kernel-filled reads, zero read() syscalls on the hot path"
status: "complete"
priority: "low"
phase: 3
depends_on: ["41-uring-selector", "42-uring-selection-probe"]
estimated_effort: "large"
created: 2026-07-03
completed: 2026-07-10
---
# Feature 43-uring-completion-mode: io_uring completion-mode read path (D14 F4)

## Description

The committed second io_uring mode: completion-based reads with kernel-filled buffer rings, sequenced strictly after the readiness parity suite is green.

## Normative sources (single source of truth — read before writing code)

- decisions/14-io-uring-reactor-backend.md — Decided Details OQ#14.2 (F4 scope; starts only after F2 parity is green)

## Scope

- CompletionSource seam next to EventReadiness (take_completions + buffer recycle protocol)
- RECV/SEND with registered buffer rings (kernel ≥5.19/6.0 probes; fallback to readiness mode)
- Transports opt read paths into inbox-pop; decoder unchanged (step over &buf[..]); per-worker-ring re-evaluation (the OQ#14.1 trigger)

## Acceptance criteria

- Hot-path reads show zero read() syscalls under strace on a supporting kernel; sub-matrix kernels fall back cleanly

---

## Gate

D14 OQ#14.2: *"F4 does not start until [F2's parity suite] is green."* When this feature was
picked up the suite did not exist, and neither did a working shared reactor. F40, F41 and F42
were reopened, repaired, and re-closed first (see their feature files). The parity suite
(`uring_epoll_parity_tests.rs`, 9 scenarios) is green on both backends. Gate satisfied.

## What shipped

### `bufring.rs` — the registered provided-buffer ring

Page-aligned array of `io_uring_buf` records registered with `IORING_REGISTER_PBUF_RING`, plus
the backing buffer space. `ProvidedBuf` is one kernel-filled buffer on loan; `Deref` gives the
bytes, `Drop` republishes the buffer id.

The ownership protocol is what makes handing out `&[u8]` sound: while a `ProvidedBuf` exists,
its id is *not* in the ring, so the kernel cannot write to it. Leaking one (`mem::forget`)
leaks a buffer from the pool — a throughput bug, not unsoundness.

`io_uring_buf_ring` is a union whose first entry's trailing `u16` **is** the ring's `tail`.
Every write touches `addr`/`len`/`bid` only; writing `resv` on entry 0 would corrupt the ring.

### `uring_completion.rs` — the completion selector

Sockets are armed with multishot `RECV` (`IORING_RECV_MULTISHOT` + `BUFFER_SELECT`) against the
buffer ring. `poll()` drains the CQ, turns recv CQEs into `ProvidedBuf`s queued per token, and
still emits an ordinary READABLE `Event` — so the reactor's wake path is byte-for-byte the same
as readiness mode, which is precisely D14's seam argument.

Three things the design had to get right:

- **Not every fd is a socket.** `RECV` returns `-ENOTSOCK` on pipes, eventfds and inotify fds.
  The selector asks `getsockopt(SO_TYPE)` once at registration and puts non-sockets on multishot
  `POLL_ADD`. Completion mode is a read-path optimisation for sockets, not a blanket swap. A
  transport that skipped `read()` on a pipe because "the reactor is in completion mode" would
  hang forever, so `is_completion_source()` is per-fd, not per-backend.
- **Buffer starvation.** When consumers hold every buffer the kernel answers `-ENOBUFS` and drops
  the multishot registration. Re-arming immediately would spin against an empty pool, so the
  token is parked and re-armed on the first `poll()` after `BufRing::recycle_count` moves — i.e.
  after a consumer actually freed a buffer.
- **Cancellation.** `POLL_REMOVE` only cancels poll requests. A `Mode::Recv` token has a
  multishot `RECV` in flight, so deregistration submits `ASYNC_CANCEL`, which matches by
  `user_data` whatever the opcode.

### `fd/completion.rs` — the `CompletionSource` seam

`CompletionSource` sits next to `EventReadiness`, exactly as D14 specifies. A task still parks on
`Depends(RegisteredFd)`; when it wakes it asks this trait whether its bytes are already in hand.
`is_completion_source()` is deliberately non-destructive — an earlier draft implemented it as
`take_completions().is_some()`, which drained the inbox and discarded the bytes.

Two consumption paths:

- **Zero-copy:** `take_completions()` → `decoder.step(&mut &buf[..])`. The Decision 12 §11
  `IncrementalDecoder` is unchanged, because `&[u8]: Read`.
- **`read`-shaped:** `RegisteredFd::read_bytes()` copies out of the inbox (one memcpy, no
  syscall) and falls back to `read(2)` on other backends. `Ok(0)` is EOF and `WouldBlock` means
  park, identically on both paths, so an existing transport adopts completion mode by changing
  one call. Partial reads across a completion are staged with an offset; the `ProvidedBuf` is
  recycled only once fully drained, and EOF latches so a dead socket never reports `WouldBlock`.

Draining the inbox clears the latched readiness — completion mode's analogue of reading until
`WouldBlock`. Without it a woken task spins instead of re-parking.

### Ladder integration

`COMPLETION_IMPLEMENTED` flipped to `true`, so `Auto` now climbs to `UringCompletion` whenever the
functional probe reports buffer rings **and** multishot recv. Below that matrix it lands on
readiness mode, and below *that*, epoll — each demotion logged with its reason. On this host
(kernel 7.0) the probe reports all four capabilities and the reactor selects completion mode.

## Acceptance: zero `read()` syscalls

`strace` was not installed, so `uring_completion_syscall_tests.rs` is its own tracer: it forks,
the child calls `PTRACE_TRACEME` and runs the hot path, and the parent single-steps it with
`PTRACE_SYSCALL`, decoding `orig_rax` and `rdi` at each syscall-entry stop. x86_64 only.

Measured:

| Path | read-family syscalls on the socket | `io_uring_enter` |
|---|---|---|
| completion `take_completions` | **0** | 1 |
| completion `read_bytes` (inbox already filled) | **0** | **0** |
| readiness mode (control) | 1 | 1 |

The control arm is load-bearing: a tracer that silently counted nothing would make the other two
pass vacuously.

Corroborated after installing `strace 7.0`: tracing the functional test shows every read-family
call lands on fd 3 (the dynamic loader reading ELF headers, `/proc/self/maps`, terminfo at
startup) and **none** on the socketpair fds. Same answer, independent mechanism. The ptrace test
is what's committed — it needs no external binary and carries its own control.

## OQ#14.1 re-evaluation (the committed trigger)

The trigger's premise was *"per-op submissions make SQ contention real"*. It is false for the
implementation that shipped: multishot `RECV` arms an fd with **one** SQE for its whole lifetime.
`submissions_are_per_registration_not_per_read` registers one socket (1 SQE), drives 64 message
round-trips, and asserts the SQE count is unchanged. It is.

**Keep the single shared reactor.** Contention moved to `BufRing`'s tail mutex (once per
`ProvidedBuf` drop, two stores, no syscall), which is fixed by a lock-free CAS if a profile ever
demands it — not by sharding rings per worker. Full reasoning recorded in D14 OQ#14.1.

## Known gap: no in-tree transport consumes the seam yet

D14 F4 lists *"transports opt their read path into inbox-pop"*. The mechanism ships and is
tested, but **nothing in-tree opts in**, because `foundation_netio` does not depend on
`foundation_nativeapis` — its transports read through `SharedByteBufferStream<RawStream>` and
never touch `RegisteredFd`. Only `foundation_http` depends on nativeapis, and only from a test.

Wiring netio's read path to the inbox means adding a `netio → nativeapis` dependency edge, which
is an architectural decision (netio is currently reactor-agnostic, and its `ReadModel::Depends`
takes readiness from the caller) with TLS interplay to think through. That is deliberately **not**
done here rather than smuggled in. The seam is ready for it: a transport swaps `read()` for
`read_bytes()`, or steps its decoder straight over `take_completions()`.

## Tests

- `uring_completion_tests.rs` (9) — delivery, recycle-on-drop, multishot persistence, EOF,
  data-then-immediate-close, non-socket poll fallback, starvation and recovery, deregistration,
  submissions-per-registration.
- `uring_completion_syscall_tests.rs` (3) — the acceptance criterion, plus its control.
- `completion_source_seam_tests.rs` (8) — the seam through the shared reactor: socket-vs-pipe
  branch, park→wake→drain, readiness clearing, 64-message no-leak round trip, EOF, and
  `read_bytes` partial reads / `WouldBlock` / latched EOF.

51 tests green under `--features uring`, 15 without it, on both build configurations.
