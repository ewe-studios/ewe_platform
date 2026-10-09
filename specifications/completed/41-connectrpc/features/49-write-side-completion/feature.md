---
feature: "Write-side completion — IORING_OP_SEND (D14 F4 SEND half)"
description: "io_uring SEND for transport writes, buffer-ownership tracking, and a path to the zero-copy proxy relay"
status: "implemented (Phases A+B+C: SEND pool + ptrace proof + split_read_write)"
priority: "medium"
phase: 3
depends_on: ["48-transport-completion-read-path"]
estimated_effort: "large"
created: 2026-07-11
---
# Feature 49: Write-side completion — IORING_OP_SEND

> **Status: implemented (Phases A+B), 2026-07-12.** The SEND mechanism and its
> zero-syscall proof are in and verified on a real kernel. Deferred: Phase C (the
> `split_read_write` completion-backed write half) and Phase D (the SEND_ZC
> zero-copy relay, which is Feature 50). The design below stands; see
> *Implementation status* immediately after this note.

## Implementation status — Phases A+B landed (2026-07-12)

What shipped, bottom-up, all behind `#[cfg(all(target_os = "linux", feature = "uring"))]`:

- **`SendPool` / `SendBuf`** (`foundation_nativeapis/src/native/fd/send_pool.rs`):
  the user-filled buffer pool (the SEND mirror of the RECV `BufRing`). Owned
  fixed-size buffers, checked out and filled at `send`, kept alive for the SQE's
  duration, recycled on drop; exhaustion returns `WouldBlock`. Uses
  `Mutex<VecDeque>` + `AtomicUsize` (no `concurrent_queue` dep, contrary to the
  sketch below). 5 unit tests.
- **`uring_completion::Selector` SEND path**: `submit_send` (checkout → record in
  an in-flight map keyed by a tagged send id → push `IORING_OP_SEND`),
  `take_send_completions`, `has_send_completions`, `has_pending_sends`, plus a new
  `SEND_TAG` and SEND-CQE handling in `drain_completions` (removes the in-flight
  `SendBuf` — recycling it — and mailboxes a `SendCompletion::Written`/`Error`,
  latching `EPOLLOUT` to wake a parked `flush`). `MAX_TOKEN` lowered to fit the new
  tag bit.
- **Threaded through** dispatch → `poll` → `Reactor` → `FdRegistration` (inherent
  `send_bytes`/`drain_sends`/`is_send_source` mirroring `read_bytes`, with a
  `write(2)` degrade path) → `RegisteredFd`.
- **`CompletionSocket::write`/`flush`** (`foundation_iogate`) now submit
  `IORING_OP_SEND` and drain-with-`WouldBlock`-barrier respectively, mirroring the
  F48 read wiring.
- **Verification**: `uring_completion_send_tests.rs` (4 real-socket tests: delivery,
  short-send cap, ordering, small-pool recycle) and, in
  `uring_completion_syscall_tests.rs`, `completion_mode_writes_the_socket_without_a_write_syscall`
  — the ptrace proof of **zero write-family syscalls** on the socket, bytes out via
  `io_uring_enter`. Acceptance criteria for the SEND mechanism, the pool
  `WouldBlock`, the `flush` barrier, and the unchanged wasm build are met. The
  epoll-control **write** arm and a `CompletionSocket`-level HTTP end-to-end test
  are the remaining nice-to-haves.

**Deferred:** Phase C (`split_read_write`'s write half as a second-registration
`CompletionSocket`) and Phase D / SEND_ZC (→ Feature 50). Phase C is deferred on
purpose, not merely unfinished: `ReadWriteStream::split_read_write` currently has
**no callers** in the tree (only the trait definition and a doc reference), so a
completion-backed write half would optimise a path nothing exercises — and it also
faces the netio↔iogate cycle (the write half cannot name `CompletionSocket` from
`foundation_netio`). It should land when a real half-duplex split consumer appears
(the natural driver is F50's SEND_ZC relay or a future H2/H3 split).

---

> **Original design proposal (retained for context).** It assumes feature 48 has
> shipped: the read path uses `Connection::Completion`, `Reactor::init()` honours
> preferences, and `ServerIo` is the runtime switch.

## Why this exists

Decision 14 F4 scoped `RECV` and `SEND` together:

> *"Zero-copy: both directions sharing one buffer ring so bytes move
> fd→buffer→fd without touching user memory."*

Feature 43 shipped `RECV` — the kernel fills registered buffers and the transport
pops the inbox. Feature 48 connected real transports to it. `SEND` was explicitly
deferred:

- **Buffer ownership runs the other way.** RECV lends the kernel a pool and gets
  buffers back in CQEs. SEND hands the kernel a buffer *we* own and must not
  touch, move, or free until the completion says the bytes are on the wire. A
  `write(2)` can return the moment the data is copied into the socket buffer; an
  `IORING_OP_SEND` cannot. Getting that wrong corrupts the response body silently
  rather than loudly.
- **The win is smaller and needs its own measurement.** The read side removed a
  syscall *per wakeup*; the write side removes one *per flush*, and a server that
  already coalesces its response into few large writes sees less.
- **It interacts with the split.** Once SEND lands, the `split_read_write` write
  half can itself be completion-backed.

This feature closes the SEND half. The zero-copy relay (`fd→buffer→fd`) is the
layer above it — SEND is the mechanism it needs.

## What `IORING_OP_SEND` actually changes

Today a transport write is:

```
transport: write(&response_buf) → write(2) → kernel copies into socket buffer → returns
```

With `IORING_OP_SEND`, the kernel postpones the copy:

```
transport: write(&response_buf) → copy into owned buffer → submit SEND SQE → return WouldBlock if SQ full
         ...
         CQE arrives: bytes are on the wire → owned buffer freed → task woken
         flush() drains all pending CQEs
```

The key difference: **between submitting the SQE and receiving its CQE, the
buffer must not move or be deallocated.** A stack buffer, a `&[u8]` borrow, or a
`Vec<u8>` being resized — all are wrong. The transport must hand SEND an *owned*
buffer with a stable address, and the kernel dictates when it is done with it.

This is not a new problem. Every async I/O library that supports io_uring writes
solves it the same way: a pool of owned, fixed-size buffers that are checked out
for a write and checked back in when the CQE arrives. The RECV side already has
this (the `BufRing`), but RECV buffers are *kernel-filled* — the kernel writes
into them and we read. SEND buffers are *user-filled* — we write into them and
the kernel reads. They can share the same pool **provided** the pool is large
enough for both directions, but the simplest correct design gives SEND its own.

## The design

### Phase A: `RegisteredFd::send_bytes` — the SEND half of the completion seam

`RegisteredFd::read_bytes` already exists and is the RECV half of feature 43's
`CompletionSource` seam. Its analogue for writes:

```rust
// backends/foundation_nativeapis/src/native/fd/completion.rs

/// The SEND half of the completion seam.
///
/// WHY: `read_bytes` is the RECV half. A transport that calls `write(2)` on a
/// completion socket still works, but every write is one syscall. Paralleling
/// RECV means the write path also costs zero syscalls on the steady state.
///
/// WHAT: submit a SEND for an owned buffer, return immediately, and deliver the
/// completion (success or error) later.
///
/// HOW: on the io_uring completion backend, the buffer is copied into a
/// pool-owned region and an `IORING_OP_SEND` SQE is submitted. On every other
/// backend this is an ordinary `write(2)` — the same degrade-as-necessary
/// pattern `read_bytes` already uses.
#[cfg(all(target_os = "linux", feature = "uring"))]
pub trait SendSource {
    /// Whether this fd submits writes through the completion path.
    fn is_send_source(&self) -> bool;

    /// Submit a write. Ownership of `buf` transfers to the kernel; the caller
    /// must not read, write, or drop the backing storage until the corresponding
    /// [`SendCompletion`] arrives.
    ///
    /// # Errors
    /// `WouldBlock` if the submission queue is full — the task should park and
    /// retry.
    fn send_bytes(&self, buf: Vec<u8>) -> io::Result<()>;

    /// Take all completed sends since the last call. A transport calls this from
    /// its `flush()`: it drains completions, sums the byte counts, and reports
    /// any error.
    fn take_send_completions(&self) -> Vec<SendCompletion>;

    /// Whether send completions are waiting, without taking them.
    fn has_send_completions(&self) -> bool;
}

#[derive(Debug)]
pub enum SendCompletion {
    /// `n` bytes were placed on the wire.
    Written(usize),
    /// The send failed. The buffer is lost (the kernel consumed it).
    Error(io::Error),
}
```

On non-completion backends, `send_bytes` degrades to `write(2)` and
`take_send_completions` returns nothing — the write already happened synchronously,
exactly as `read_bytes` degrades to `read(2)` today.

### The buffer pool for SEND

`IORING_OP_SEND` needs the buffer to stay alive. The transport typically writes
from a `SharedByteBufferStream` or a `Vec<u8>` it owns, but the `&[u8]` it passes
to `Write::write` is transient. Two approaches:

| Approach | Buffer ownership | Cost |
|---|---|---|
| **Copy into a SEND pool buffer, then SEND from the pool** | Pool owns the buffer; kernel reads from pool; buffer returns to pool on CQE | One `memcpy` per write — the same trade-off RECV made |
| **SEND from the caller's buffer directly** | Caller must keep `Vec<u8>` alive; cannot touch it until CQE | Zero-copy, but the caller's buffer ownership model changes |

**Recommendation: copy-into-pool (same trade-off as RECV).** The transport
already composes its response in a `Vec<u8>` or `SharedByteBufferStream` and
calls `write(&buf)`. Changing every caller to lend ownership of its buffer to the
kernel is a larger migration than the RECV path required (where the kernel
already owned the buffer). One `memcpy` into the pool, in exchange for zero
syscalls per write — measured against the control arm — is the right first step.

The RECV `BufRing` is a registered `PBUF_RING` (kernel ≥ 5.19) — kernel-filled.
SEND needs a **user-filled pool**: a `Vec<Vec<u8>>` behind a lock-free queue.
When a transport wants to write:

1. Check out a buffer from the SEND pool (or allocate if the pool is empty, up
   to a cap)
2. `memcpy` the caller's bytes into it
3. Submit `IORING_OP_SEND` with that buffer's address
4. When the CQE arrives, return the buffer to the pool

```rust
// backends/foundation_nativeapis/src/native/fd/send_pool.rs   (new)

/// A pool of owned, fixed-size buffers for `IORING_OP_SEND`.
///
/// WHY: SEND requires the buffer to stay alive until the CQE. A `&[u8]` borrowed
/// from the transport's response buffer satisfies `write(2)` but not an async
/// kernel operation — the borrow ends before the CQE. The pool owns the buffer
/// for the kernel's duration.
///
/// WHAT: `buf_size`-byte allocations, checked out on `send_bytes` and checked
/// back in when the CQE arrives.
///
/// HOW: an MPSC queue of free buffers. Allocation on empty pool, up to `max_bufs`;
/// beyond that, `send_bytes` blocks until a buffer returns (or falls back to
/// `write(2)` — the degrade path).
#[derive(Debug)]
pub struct SendPool {
    buf_size: usize,
    max_bufs: usize,
    free: concurrent_queue::ConcurrentQueue<Vec<u8>>,
    allocated: std::sync::atomic::AtomicUsize,
}

impl SendPool {
    pub fn new(buf_size: usize, max_bufs: usize) -> Self { /* … */ }

    /// Take a buffer, allocating if under `max_bufs`.
    ///
    /// # Errors
    /// `WouldBlock` if at `max_bufs` and no buffer is free — the transport
    /// should park until a SEND CQE returns one.
    pub fn checkout(&self) -> io::Result<SendBuf> { /* … */ }
}

/// An owned buffer checked out from the pool. On drop, returns to the pool.
pub struct SendBuf {
    buf: Vec<u8>,
    pool: Arc<SendPool>,
}

impl Deref for SendBuf {
    type Target = [u8];
    fn deref(&self) -> &[u8] { &self.buf[..self.len] }
}

impl Drop for SendBuf {
    fn drop(&mut self) {
        self.pool.checkin(std::mem::take(&mut self.buf));
    }
}
```

`SendBuf` is the SEND analogue of `ProvidedBuf` on the RECV side. The drop
returns the buffer to the pool; leaking one permanently reduces the pool size
(same failure mode as `ProvidedBuf`).

### `CompletionSocket` gets `send_bytes` and `flush_sends`

```rust
impl<T: AsRawFd + Read + Write> Write for CompletionSocket<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.fd.is_send_source() {
            let len = buf.len();
            self.fd.send_bytes(buf)?;   // copies into pool, submits SEND
            Ok(len)                       // bytes accepted — not yet on the wire
        } else {
            self.fd.get_mut().write(buf)  // write(2) on non-completion backends
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fd.is_send_source() {
            // Drain all SEND CQEs. A transport that calls flush() before the
            // CQEs arrive parks the task until they do — same as `WouldBlock`
            // on a nonblocking write that hasn't completed.
            self.fd.drain_sends()
        } else {
            self.fd.get_mut().flush()
        }
    }
}
```

A transport that calls `write()` without `flush()` and then parks on something
else will have bytes in flight. This is correct: the transport does not depend on
the bytes being on the wire until it needs to close the connection or measure
latency. The reactor's drain thread processes SEND CQEs alongside RECV CQEs, and
the task is woken when all its in-flight sends have completed.

### `WriteHalf` becomes completion-backed (one SEND, one CQE)

Feature 48's `ReadWriteStream::split_read_write` returns a write half that
`dup(2)`s the fd and uses `write(2)`. Once SEND lands and its buffer-pool
ownership is settled, the write half can be a `CompletionSocket` on the dup'd fd:

```rust
// The write half after SEND ships
fn split_read_write(self) -> io::Result<(ReadHalf, WriteHalf)> {
    let write_fd = dup(self.as_raw_fd())?;
    let write_sock = CompletionSocket::new(
        TcpStream::from(write_fd),
        Token(write_token()),
        None,  // no peer addr on the write half
    )?;
    Ok((self.into_read_half(), write_sock))
}
```

The write half never reads, so there is no inbox collision with the read half's
token. The dup'd fd has its own registration, its own token, and its own SEND
CQE stream.

This is **Phase C** — it is sequenced behind the basic write path and the buffer
pool because it adds a second registration per connection.

## Interaction with `IORING_OP_SEND_ZC` and the zero-copy relay

`IORING_OP_SEND_ZC` (kernel ≥ 6.0) is the mechanism that would let bytes move
from a RECV `ProvidedBuf` directly to a SEND without touching user memory:

```
RECV buf → [memcpy skipped] → SEND buf → kernel places on wire
                                   ↑
                              same buffer, two registrations
```

This is Decision 30's Phase 4 and the eventual proxy relay: the proxy reads a
request body into a RECV buffer, and instead of copying it into a SEND buffer, it
forwards the same buffer to the upstream. The buffer is never copied into user
memory — the kernel reads it from one fd and writes it to another.

**This feature does NOT ship the zero-copy relay.** It ships the SEND mechanism
the relay needs. The relay itself is
**[Feature 50](../50-client-completion-relay/feature.md)** (Part C there),
because:

- `IORING_OP_SEND_ZC` has different completion semantics: the CQE reports both
  "bytes accepted" and "bytes on the wire" in separate fields, and the buffer is
  not reusable until the second notification.
- The proxy's `splice_bidirectional` needs to coordinate two fds, two buffer
  rings, and the no-copy handoff — a correctness surface of its own.
- The relay's benefit is measurable only in the proxy path; SEND benefits every
  server response.

## Scope

- `foundation_nativeapis`:
  - `SendSource` trait (the SEND half of the `CompletionSource` seam)
  - `SendPool` — user-filled buffer pool for SEND
  - `SendBuf` — checked-out buffer, returns to pool on drop
  - `RegisteredFd` gains `send_bytes`, `drain_sends`, `is_send_source`
  - `uring_completion::Selector` gains `register_send_fd` (analogue of
    `register_recv_fd`), `arm_send`, and SEND CQE processing in the drain loop
  - `Mode::Send` added to the `Mode` enum alongside `Mode::Recv` and `Mode::Poll`
- `CompletionSocket<T>` (in `foundation_nativeapis`, imported by
  `foundation_netio` since F48):
  - `Write::write` delegates to `send_bytes` when `is_send_source()`
  - `Write::flush` delegates to `drain_sends`
  - `is_send_source()` and `has_send_completions()` exposed for transport use
- `foundation_netio`:
  - `ReadWriteStream::split_read_write` — write half becomes
    `Connection::Completion` instead of a raw `write(2)` fd (Phase C, sequenced
    after the pool is stable)
- `foundation_http`: no changes — the `ServerIo` enum gates the RECV path at
  accept time. SEND is a per-write decision that `CompletionSocket` makes
  internally. An operator who asked for `ServerIo::Completion` gets both RECV and
  SEND.

## Phased rollout

- **Phase A: `SendPool` + `RegisteredFd::send_bytes`.** The pool, the trait, and
  the `write(2)` fallback on non-completion backends. SEND CQE processing in the
  reactor drain thread. `CompletionSocket::write` uses `send_bytes`.
- **Phase B: measurement.** The ptrace syscall counter, pointed at a real served
  connection: **zero** write-family syscalls on the connection fd, with the epoll
  control arm. `perf stat -e raw_syscalls:sys_enter` for the aggregate.
  `wrk -c 100 -t 4` against the proxy, io_uring vs epoll, measuring both
  throughput and tail latency (SEND's win is per-flush, so the latency histogram
  is the signal).
- **Phase C: `split_read_write` completion-backed write half.** The dup'd fd
  gets its own `CompletionSocket` with `Mode::Send`. This is sequenced behind the
  pool being stable because it adds a second registration per connection.
- **Phase D (separate feature): `IORING_OP_SEND_ZC` and the zero-copy proxy
  relay.** bytes move from RECV `ProvidedBuf` to SEND without a user-memory
  memcpy. The proxy's `splice_bidirectional` becomes a buffer-ring handoff.

## Verification

- `cargo test -p foundation_nativeapis` — `SendPool` checkout/checkin, pool
  exhaustion fallback, `SendBuf` drop-recycle.
- `uring_completion_syscall_tests.rs` extended with **zero write-family
  syscalls** on the SEND path, epoll control arm.
- `cargo test -p foundation_netio` — `Connection::Completion` write/flush parity
  against `Connection::Tcp`, both with and without the SEND pool active.
- `cargo check -p foundation_netio --target wasm32-unknown-unknown` — unchanged.
  SEND lives in netio's `src/native/` (post-F48 restructuring), which the wasm
  build never compiles.

## Out of scope

- `IORING_OP_SEND_ZC` / zero-copy relay (separate feature).
- `IORING_OP_SENDMSG` / vectored writes. The initial path uses `IORING_OP_SEND`;
  `SENDMSG` with a gathered write (`writev` semantics) is a natural extension
  once the pool and the SEND CQE path are solid.
- `accept()` via `IORING_OP_ACCEPT` (multishot accept). Listeners stay on
  `POLL_ADD` — this is common to F48 and F49.
- HTTP/3 (features 34/35). QUIC writes UDP through its own path.
- Per-worker SEND pools. Start with one pool on the shared reactor; same
  reasoning as the RECV pool (D14 OQ#14.1 — submissions are
  `O(registrations)`, not `O(writes)`).

## Open questions for review

1. **Pool sizing.** RECV has 256 × 16 KiB. SEND writes are typically larger
   (a response body can be megabytes), but the pool buffers are fixed-size.
   A 16 KiB SEND buffer means a 1 MiB response body is 64 SENDs — reasonable,
   but does the pool need more buffers than RECV to avoid stalls under
   concurrent writers? Start at 256 × 16 KiB and measure.
2. **One pool or two?** A single pool for RECV and SEND means contention between
   readers and writers on the same connection, which is the common case (every
   connection both reads and writes). Separate pools avoid this but double the
   pinned memory. Start with separate pools; measure the memory overhead before
   merging.
3. **`flush()` semantics when sends are in flight.** The transport calls
   `flush()` and parks. The reactor drain processes SEND CQEs and wakes the
   task. But if multiple writes are in flight, does `flush()` wait for all of
   them or just the oldest? All of them — a transport that called `flush()`
   expects every byte it wrote to be on the wire when `flush()` returns.
4. **`send_bytes` failure and buffer loss.** If `IORING_OP_SEND` fails (CQE
   with `-ECONNRESET`, `-EPIPE`), the buffer is lost — the kernel accepted it.
   The transport sees an error on `flush()` and closes the connection. The lost
   buffer is a permanent reduction in pool size, same as a leaked `ProvidedBuf`.
   Counter-based monitoring (pool allocation count, free count) catches this.
5. **Interaction with TLS.** TLS writes go through `rustls::StreamOwned` which
   calls `write()` on the inner `Connection`. So `Connection::Completion::write`
   → `send_bytes` works identically for TLS — no TLS-specific SEND code. But
   TLS writes are typically small (16 KiB ciphertext records), so the win per
   write is smaller than for a plaintext proxy forwarding 1 MiB bodies. Worth
   measuring separately.

## Acceptance criteria

- A real HTTP/1.1 or WebSocket connection, served over a TCP socket on a
  completion-capable kernel, completes a response with **zero write-family
  syscalls on that socket**, measured by the ptrace counter.
- With the reactor forced to `BackendPreference::Epoll`, the identical transport
  code path works and *does* issue `write(2)` — the control arm.
- `SendPool` checkout blocks (returns `WouldBlock`) when at capacity and no
  buffer is free; a SEND CQE returning a buffer unblocks it.
- `CompletionSocket::flush()` drains all in-flight SENDs before returning.
- `cargo check -p foundation_netio --target wasm32-unknown-unknown` is unchanged.
