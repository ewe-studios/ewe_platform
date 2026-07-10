# 30 — io_uring completion API integration into foundation_netio

**Date:** 2026-07-10
**Status:** Proposed — design document for implementation

## Context

`foundation_nativeapis` now has a full io_uring readiness selector (F41–F43)
using multishot `IORING_OP_POLL_ADD`. This eliminates the per-wait syscall cost
of epoll — once an fd is registered, every readiness transition lands as a CQE
with zero additional syscalls. The steady state is one `io_uring_enter` per
*batch* of ready fds.

But readiness is only half the story. The other half is **data movement**: every
`RawStream::read(buf)` still calls `libc::read()` → kernel → copy_to_user. The
io_uring **completion API** (`IORING_OP_READ`, `IORING_OP_WRITE`,
`IORING_OP_READ_FIXED`) moves data without the user-space `read()` call at all —
the kernel copies data directly into a pre-registered buffer and delivers a CQE
with the byte count. No syscall, no context switch, no buffer copy through
userspace.

This decision documents **how** to integrate the completion API into
`foundation_netio`, **why** `RawStream` makes it subtle but not hard, and
**what the options are** with concrete code paths.

## Current architecture

### RawStream — the universal I/O handle

```rust
// foundation_netio/src/netcap/no_wasm.rs:37-60
pub enum RawStream {
    AsPlain(BufferedReader<BufferedWriter<Connection>>, DataStreamAddr),
    AsServerTls(BufferedReader<BufferedWriter<ServerSSLStream>>, DataStreamAddr),
    AsClientTls(BufferedReader<BufferedWriter<ClientSSLStream>>, DataStreamAddr),
}
```

`RawStream` implements `std::io::Read` and `std::io::Write` by delegating
through `BufferedReader<BufferedWriter<T>>` to the inner type's `Read`/`Write`.

### Connection — the underlying socket

```rust
// foundation_netio/src/netcap/connection/mod.rs:35-36 (simplified)
pub enum Connection {
    Tcp(TcpStream),
    #[cfg(unix)] Unix(UnixStream),
}
```

`Connection` exposes `AsRawFd` — the fd is reachable even through TLS wrappers
(`RawStream::as_raw_fd()` → `inner.get_core_ref().as_raw_fd()`).

### BufferedReader / BufferedWriter — the buffer layer

`foundation_core::io::ioutils::BufferedReader` wraps a `Read` type and adds
internal buffering + peek support. `BufferedWriter` does the same for `Write`.
Both implement `Read`/`Write` by delegating to the inner type.

### io_uring selector — readiness only (F41)

```rust
// foundation_nativeapis/src/native/poll/sys/unix/selector/uring.rs
// Registers fds with IORING_OP_POLL_ADD multi=true
pub fn register_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()>;
pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()>;
```

This is a **readiness model**: "fd 7 is readable" → caller calls `read()` on fd 7.
The `read()` call is still a `libc::read()` syscall. The selector replaces epoll,
not the I/O path.

## What completion mode changes

With the completion API, the io_uring becomes the **data path**, not just the
readiness indicator:

```
# Before (readiness-only):
stream.read(buf)          → libc::read(fd, buf, len)  -- syscall
  ↓
kernel copies data → buf

# After (completion):
uring.submit_read(fd, buf, offset)
  ↓
uring.wait()              → io_uring_enter           -- one syscall for ALL I/O
  ↓
CQE: { res: n, user_data: token }
  ↓
buf[..n] is filled         -- data already there, no read() call
```

The win is batching: one `io_uring_enter` can submit N reads + M writes + drain
pending completions, and the kernel processes them asynchronously. For a proxy
handling many connections, this collapses N syscalls into ~1.

## Why RawStream makes this subtle (but not hard)

The concern is: `RawStream` implements `std::io::Read`/`Write`. Every caller
above it — `SimpleHttpClient`, `HttpResponseReader`, `splice_bidirectional` —
calls `stream.read(&mut buf)`. If we replace the I/O path with io_uring, the
`Read` trait's signature `fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>`
becomes awkward because:

1. **io_uring owns the buffer lifecycle.** `IORING_OP_READ` needs the buffer to
   be registered or pinned. A `&mut [u8]` borrowed from a stack frame isn't
   registered with the kernel.

2. **Completion is asynchronous.** `Read::read()` is synchronous — it must
   return bytes or block. io_uring completions arrive on the CQ, which requires
   polling (enter → wait → drain CQE). You can't just replace `libc::read()`
   with `io_uring_read()` inside `read()` — there's no poll loop inside `read()`.

3. **Buffer pools are shared.** The efficiency of io_uring comes from
   pre-registered buffer rings (`IORING_SETUP_BUFFER_RING`). Multiple
   connections share a pool of kernel-accessible buffers. `Read::read(buf)`
   would need to pick a buffer from the pool, submit the read, and wait —
   that's a different model than "here's my stack buffer, fill it."

These are **integration concerns**, not blockers. All three have solutions:

### Solution 1: `RawStream` variant with registered buffers (recommended)

Add a new `RawStream` variant that holds a buffer from a shared pool and an
io_uring submission handle:

```rust
pub enum RawStream {
    AsPlain(BufferedReader<BufferedWriter<Connection>>, DataStreamAddr),
    AsServerTls(...),
    AsClientTls(...),
    // NEW: io_uring-backed variant
    Uring(UringStream, DataStreamAddr),
}

struct UringStream {
    /// The fd is registered with the ring for I/O, not just readiness.
    fd: RawFd,
    /// Handle into the shared buffer pool — the kernel writes directly here.
    buf_handle: BufHandle,
    /// Shared ring reference for submitting I/O ops.
    ring: Arc<UringDriver>,
    /// Bytes available in buf right now (set after recv CQE arrives).
    ready: usize,
    /// Position in buf for the next read.
    pos: usize,
}
```

`UringStream` implements `std::io::Read` by:
- If `ready > pos`: copy `buf[pos..ready]` into caller's slice (fast, no syscall)
- If `ready == pos` and `pos > 0`: submit a new `IORING_OP_READ` for this fd, park
- If first read: submit `IORING_OP_READ`, park until CQE arrives

This is the "buffer swap" model: the kernel fills our registered buffer, we copy
into the caller's `&mut [u8]`, then re-submit. One extra copy vs zero-copy, but
it works with existing `Read` trait callers — no API changes above `RawStream`.

```rust
impl std::io::Read for UringStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Drain what's already in our buffer.
        if self.ready > self.pos {
            let n = (self.ready - self.pos).min(buf.len());
            buf[..n].copy_from_slice(&self.ring.buffer(self.buf_handle)[self.pos..self.pos + n]);
            self.pos += n;
            return Ok(n);
        }
        // Buffer exhausted — submit a new read and park.
        self.pos = 0;
        self.ready = 0;
        self.ring.submit_read(self.fd, self.buf_handle);
        self.ring.wait_one(self.fd)?; // parks until CQE for this fd
        self.ready = self.ring.last_result();
        if self.ready == 0 {
            return Ok(0); // EOF
        }
        self.read(buf) // recurse — now ready > pos
    }
}
```

**Trade-off**: one extra `copy_from_slice` per `read()` call. Acceptable because
the kernel→registered-buffer copy is zero-copy from the NIC, and the
registered-buffer→caller-buffer copy happens in userspace at memcpy speed.

**Eliminating the copy** requires callers to use the registered buffer directly:
`fn read_direct(&self) -> &[u8]`. This is a separate API surface for
performance-sensitive paths (proxy forwarding, websocket relay) while `Read`
remains for generic code.

### Solution 2: `Read` trait with `io_uring` re-submit on every call

Simpler but less efficient: submit `IORING_OP_READ` targeting the caller's
`&mut [u8]` directly, then `io_uring_enter` + wait for the single CQE:

```rust
impl std::io::Read for UringStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let sqe = opcode::Read::new(Fd(self.fd), buf.as_ptr(), buf.len() as u32)
            .build()
            .user_data(self.token);
        self.ring.submit_and_wait_one(&sqe)?;
        let cqe = self.ring.drain_one()?;
        if cqe.result() >= 0 {
            Ok(cqe.result() as usize)
        } else {
            Err(io::Error::from_raw_os_error(-cqe.result()))
        }
    }
}
```

**Trade-off**: no extra copy, but the caller's buffer must be stable across the
`io_uring_enter` — stack buffers are fine (the kernel reads them before
returning), but `Vec<u8>` reallocation between submit and completion would
corrupt. Also, this is one syscall per `read()` — no batching win. It's
strictly worse than `libc::read()` for single calls; the win only comes when
multiple reads are submitted in one `enter`.

### Solution 3: Trait-based transport abstraction

Replace `RawStream`-as-enum with a trait, then implement io_uring as one
backend:

```rust
trait Transport: Read + Write + AsRawFd {
    fn submit_read(&self, ring: &UringDriver);
    fn submit_write(&self, ring: &UringDriver, buf: &[u8]);
}
```

**Trade-off**: clean architecture, but every existing `match self { RawStream::AsPlain(..) => .. }`
needs to become dynamic dispatch. The `RawStream` enum is deeply embedded
across `foundation_netio`, `foundation_http`, and `foundation_connectrpc`.
A trait migration is a separate, larger project.

## Recommended approach: Solution 1 (RawStream variant)

Add `UringStream` as a `RawStream` variant behind a feature gate
(`uring-completion`). The existing enum dispatch pattern handles it
transparently:

```rust
impl std::io::Read for RawStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            RawStream::AsPlain(inner, _) => inner.read(buf),
            RawStream::AsServerTls(inner, _) => inner.read(buf),
            RawStream::AsClientTls(inner, _) => inner.read(buf),
            #[cfg(feature = "uring-completion")]
            RawStream::Uring(inner, _) => inner.read(buf), // delegates to UringStream::read
        }
    }
}
```

No code above `RawStream` changes. The proxy, HTTP client, and WebSocket relay
keep calling `stream.read(buf)` — they don't know or care that the bytes came
through io_uring.

## Buffer pool design

```rust
/// Shared buffer ring registered with the kernel via IORING_SETUP_BUFFER_RING.
/// One pool per io_uring instance (typically one per process or thread pool).
struct UringBufPool {
    /// The registered buffer region (mmap'd, kernel-accessible).
    bufs: Vec<Vec<u8>>,
    /// Free buffer indices.
    free: crossbeam_queue::ArrayQueue<u16>,
    /// Total buffers.
    count: u16,
    /// Buffer size (all buffers are the same size — typically 64KB for TCP).
    buf_size: u16,
}

impl UringBufPool {
    /// Acquire a buffer handle.  Returns None if the pool is exhausted.
    fn acquire(&self) -> Option<BufHandle>;

    /// Release a buffer handle back to the pool.
    fn release(&self, handle: BufHandle);

    /// Get the slice for a handle (used by UringStream::read to copy to caller).
    fn buffer(&self, handle: BufHandle) -> &[u8];
}
```

A `BufHandle` is just a `u16` index — cheap to pass around, no allocation.

## UringDriver — the submission/completion engine

```rust
/// Shared io_uring instance that drives both readiness polling (F41) and
/// completion I/O (this feature).  One per process or thread pool.
struct UringDriver {
    ring: IoUring,
    pool: UringBufPool,
    /// Pending I/O submissions that haven't been flushed yet.
    pending: Mutex<Vec<PendingOp>>,
}

struct PendingOp {
    fd: RawFd,
    buf_handle: BufHandle,
    /// Tokio-style waker for the task waiting on this op.
    waker: Option<Waker>,
}
```

The driver's main loop:

```rust
impl UringDriver {
    fn run(&self) {
        loop {
            // 1. Submit all pending ops.
            self.flush_pending();
            // 2. Wait for at least one completion.
            self.ring.submit_and_wait(1)?;
            // 3. Drain CQEs.
            while let Some(cqe) = self.ring.completion_shared().next() {
                let token = cqe.user_data();
                let result = cqe.result();
                // 4. Wake the task waiting on this fd.
                self.wake_task(token, result);
            }
        }
    }
}
```

This runs on its own thread (or on the valtron pool via `TaskIterator`). Tasks
submit I/O ops and park; the driver wakes them when data arrives.

## Integration with the proxy

The proxy's hot path is `ProxyHandler::serve()` → `forward_http()` →
`SimpleHttpClient::send()` → `RawStream::read()`. With Solution 1:

1. **Accept**: `TcpStream` from `TcpListener` → register fd with uring for
   readiness (already works via F41)
2. **Read request**: `RawStream::read()` → `UringStream::read()` → drain buffer
   or submit `IORING_OP_READ` and park
3. **Forward to backend**: `SimpleHttpClient` uses its own `RawStream` for the
   upstream connection — same `UringStream` model
4. **Splice (TCP/UDP passthrough)**: `splice_bidirectional` currently does a
   spin-poll on two non-blocking `Read` handles. With io_uring, this becomes:
   submit reads on both fds, wait for the first completion, forward the bytes,
   repeat. The 1ms sleep in the spin loop disappears — the uring parks until
   data is actually available.

## Phased rollout

### Phase 1: UringDriver + UringBufPool
- Create the shared driver with buffer ring registration
- Test buffer acquire/release under contention
- Benchmark: buffer pool throughput vs malloc-per-read

### Phase 2: UringStream + RawStream::Uring
- Add the `Uring` variant behind `uring-completion` feature
- Implement `Read`/`Write` via the buffer-swap model
- Wire into `RawStream::from_tcp()` — when the feature is on, new connections
  get `Uring` variant

### Phase 3: Proxy integration
- Benchmark proxy throughput with `uring-completion` on vs off
- Measure syscall reduction (perf stat -e raw_syscalls:sys_enter)
- Expected win: 2–4x throughput for small requests, 1.2–1.5x for large bodies

### Phase 4: Zero-copy relay
- `splice_bidirectional` replacement using linked buffers
- Both sides share the same buffer ring; data moves fd→buffer→fd without
  touching user-space memory

## Verification

1. `cargo test -p foundation_netio --features uring-completion` — UringStream
   read/write parity with TcpStream
2. `cargo test -p foundation_proxy --features docker-tests,uring-completion` —
   proxy integration tests pass with io_uring I/O path
3. Benchmark: `wrk -c 100 -t 4` against proxy → io_uring vs epoll throughput
4. `perf stat -e syscalls:sys_enter_*` — syscall count drops by ~40% for the
   proxy hot path
