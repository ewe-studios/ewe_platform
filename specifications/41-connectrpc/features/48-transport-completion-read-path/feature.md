---
feature: "Transport opt-in to the completion read path (D14 F4 remainder)"
description: "Give netio transports a byte source backed by the io_uring completion inbox, so real connections read with zero read() syscalls"
status: "proposed"
priority: "medium"
phase: 3
depends_on: ["43-uring-completion-mode"]
estimated_effort: "large"
created: 2026-07-10
---
# Feature 48: Transport opt-in to the completion read path

> **Status: proposed, for review.** This document is a design proposal, not a
> plan of record. It ends with a recommendation and the alternatives I rejected,
> with the reasons. Nothing is implemented.
>
> **Supersedes and absorbs** spec-53 Decision 30 (`30-iouring-completion-netio.md`),
> which is merged into this document and deleted. See *Relationship to Decision 30*
> below: its problem statement stands, several of its premises were overtaken by
> feature 43, and its recommended shape is not the one recommended here. Its proxy,
> splice, phasing and benchmark work is carried forward.

## Why this exists

Decision 14 F4 scoped three things. Two shipped in feature 43:

1. the `CompletionSource` seam in `foundation_nativeapis`, and
2. `RECV` with registered buffer rings — measured at **zero** `read()` syscalls
   on the hot path.

The third did not:

> *"Transports opt their read path into inbox-pop; decoder unchanged (step over
> `&buf[..]`)."*

Nothing in the tree opts in. `foundation_netio` does not depend on
`foundation_nativeapis`; its transports read through
`SharedByteBufferStream<RawStream>` and never touch `RegisteredFd`. So the
kernel-read path exists, is tested, and serves no production connection. This
feature closes that.

## The question this design has to answer

> *"Truthfully all the things RawStream takes are Fd, so unsure why it can't
> work with `SharedByteBufferStream<RawStream>`."*

It can, and that observation is the key to the whole design. Nothing about
`SharedByteBufferStream` is in the way:

```rust
// backends/foundation_core/src/io/ioutils/mod.rs:949
pub struct SharedByteBufferStream<T: Read>(OwnedReader<ByteBufferPointer<T>>);
```

It is generic over **`T: Read`**. It does not care where bytes come from. And
`RawStream: Read` already. So `SharedByteBufferStream<RawStream>` keeps working
verbatim — *provided the bytes reaching `RawStream::read` come from the
completion inbox instead of a `read(2)`*.

The real question is therefore not "can `SharedByteBufferStream` do it" but
**at which layer do we swap the byte source**, and **who owns the reactor
registration**. Look at the layering:

```
  SharedByteBufferStream<RawStream>          generic over T: Read      ✅ no change
    └── RawStream                            enum, 3 variants
          ├── AsPlain(BufferedReader<BufferedWriter<Connection>>)
          ├── AsServerTls(BufferedReader<BufferedWriter<ServerSSLStream>>)
          └── AsClientTls(BufferedReader<BufferedWriter<ClientSSLStream>>)
                                                          │
                                                          ▼
                    RustlsStream<T>(Arc<Mutex<rustls::StreamOwned<T, Connection>>>)
                                                                          │
                                                                          ▼
  Connection                                 enum: Tcp | Unix | Tls   ← the real seam
    ├── Tcp(TcpStream)          ──► read(2)
    ├── Unix(UnixStream)        ──► read(2)
    └── Tls(TlsStream)          ──► reads *through* a Connection
```

Two facts fall out, and they decide the design.

**Fact 1: `Connection` is the only place bytes enter the process.**
`impl Read for Connection` (`netcap/connection/mod.rs:539`) is the single
`read(2)` for every plaintext transport.

**Fact 2: TLS reads *through* `Connection`.**
`RustlsStream<T>(Arc<Mutex<rustls::StreamOwned<T, Connection>>>)` — rustls'
`StreamOwned<C, S>` is generic over its socket `S`, and we instantiate `S =
Connection`. So a completion-backed `Connection` gives **TLS completion mode for
free**, with no TLS-specific work at all. rustls consumes ciphertext through
`Read`; it does not care that the ciphertext arrived in a kernel-filled buffer.

That is the whole answer to "why can't it work". It can. We change one enum,
one layer down from where the question was aimed. `RawStream` needs **no new
variant**, and no fd faking.

## What actually blocks it

Not the types. The **dependency direction** and the **registration lifecycle**.

- `foundation_netio` is reactor-agnostic and wasm-capable. `foundation_nativeapis`
  is native-only (`crate-type = ["rlib", "cdylib"]`, `libc`, `io-uring`). Making
  netio depend on it unconditionally drags a native reactor into every wasm
  build.
- Someone must register the fd with the shared reactor, choose the completion
  path, and deregister on close. Today netio's `ReadModel::Depends` receives
  readiness *from the caller* precisely so netio owns none of that.
- Only `foundation_http` depends on `foundation_nativeapis` today, and only from
  a test.

## Relationship to Decision 30

Decision 30 (`30-iouring-completion-netio.md`, same date) asked the same question
and reached a different answer. It was written against the tree *before* feature
43 merged, and four of its premises no longer hold:

| Decision 30 says | Actually true after F43 |
|---|---|
| "`foundation_nativeapis` now has a full io_uring **readiness** selector (F41–F43)… The selector replaces epoll, not the I/O path." | F43 *is* the completion path. `uring_completion::Selector` arms multishot `RECV` against a registered buffer ring and delivers bytes. Measured at 0 read syscalls. |
| Proposes building `UringBufPool` (a `Vec<Vec<u8>>` + `ArrayQueue<u16>`). | `BufRing` exists: a page-aligned `io_uring_buf` ring registered with `IORING_REGISTER_PBUF_RING`, with `ProvidedBuf` returning buffers on `Drop`. A `Vec<Vec<u8>>` is *not* a kernel buffer ring; the kernel needs one contiguous, page-aligned, registered region. |
| Proposes `UringDriver` with its own thread and `wake_task(token)`. | The shared `Reactor` already owns a drain thread, a `Token → Ready` cache, and the `EventReadiness` wake. A second driver would be a second reactor. |
| Proposes `IORING_OP_READ` per read. | `RECV_MULTISHOT` arms **once per fd** for its lifetime. This is why D14 OQ#14.1's SQ-contention worry did not materialise: submissions are `O(registrations)`, not `O(reads)` — measured, 64 round-trips push zero extra SQEs. |
| `Connection` is `Tcp | Unix`. | It is `Tcp | Unix | Tls`, and `RustlsStream<T>` wraps a `Connection`. That is the fact this design turns on. |

Its `UringStream::read` also **parks inside `Read::read`** (`self.ring.wait_one(fd)?`
then recurses). That inverts the valtron model: a leaf task must return
`Depends(source)` and be re-polled, not block a worker thread inside a `read`.
`RegisteredFd::read_bytes` instead returns `WouldBlock`, and the task parks — the
same contract as `read(2)` on a nonblocking socket, which is precisely why no
transport code needs to change.

**What Decision 30 got right, and this document keeps:**

- The framing: readiness is half the story; data movement is the other half.
- That `RawStream`'s `Read` impl is the compatibility surface, and nothing above
  it should change.
- The buffer-swap trade-off: one `memcpy` from the registered buffer into the
  caller's `&mut [u8]`, in exchange for working with every existing `Read` caller.
  Its "eliminating the copy requires `fn read_direct(&self) -> &[u8]`" is exactly
  this document's open question 3 (`take_completions()` zero-copy).
- Its rejection of per-read submission (its Solution 2) and of a wholesale trait
  migration (its Solution 3), for the same reasons.
- The proxy integration, `splice_bidirectional`, phasing, and benchmark plan —
  carried forward below.

**Where it differs, and why this document overrides it:** Decision 30's Solution 1
adds a `RawStream::Uring` variant. That is one layer too high. See *Considered and
unnecessary: expanding `RawStream`* — it leaves both TLS variants unable to use
completion mode, whereas seating the source at `Connection` gives TLS the path for
free, because `RustlsStream` reads through a `Connection`.

## Constraints the design must respect

1. **Byte transparency by default.** Feature 43 already learned this the hard
   way: arming `RECV` behind a caller's back makes the kernel drain the socket
   and the caller's own `read` find nothing. `Connection::Tcp` must keep
   behaving exactly as it does now.
2. **Not every fd can receive.** `RECV` is a connected-socket operation.
   Listening sockets (`SO_ACCEPTCONN`) and non-sockets fall back to `POLL_ADD`.
   `accept()` never goes through completion mode.
3. **Writes are unchanged.** F43 shipped `RECV` only. `SEND` is future work;
   `Connection::write` stays a `write(2)`.
4. **The pool is finite.** 256 × 16 KiB buffers. A `ProvidedBuf` held by a slow
   consumer is a buffer the kernel cannot fill. Copy out and drop.
5. **wasm and non-Linux must not notice.** Default feature set unchanged.

---

## Recommendation: a trait-object byte source in `Connection` (Option B)

Add a variant to `Connection` that holds a boxed byte source. netio defines the
trait; **netio gains no new dependency**. The composition root — whoever already
depends on `foundation_nativeapis` — constructs the completion-backed
implementation and hands it in.

### What `Connection` actually demands

A new variant is not free. `Connection` carries more than `Read + Write`, and
every one of these needs a `Source` arm. This is the real cost of the feature,
and it is where the design has to be honest:

| Obligation | On the completion path |
|---|---|
| `Read` | **The whole point.** Pops the inbox; no syscall. |
| `Write` | Ordinary `write(2)`. `SEND` is out of scope. |
| `AsRawFd` / `AsFd` | Trivial — the source owns the fd. |
| `PeekableReadStream::peek` | **Easier than on a socket.** The staged `ProvidedBuf`s already hold the bytes, so `peek` reads them without draining. TCP needs `MSG_PEEK`; we need a slice. |
| `ReadTimeoutOperations` | Timeouts are a socket-option concept. In completion mode the read never blocks — it returns `WouldBlock` and the task parks — so `set_read_timeout` is a no-op that must be *documented*, not silently ignored. |
| `peer_addr` / `local_addr` / `stream_addr` | `getpeername`/`getsockname` on the owned fd. |
| `shutdown` | `shutdown(2)` on the fd. |
| `SplitReadStream::split_connection` | **The hard one — see below.** |
| `try_clone` | Same problem as split. |

#### `split_connection` is the one genuinely hard obligation

`Connection::split_connection` returns *another `Connection`* by `try_clone`ing
the fd, so a transport can read on one half and write on the other. H1 relies on
this (feature 23: `open()` spawns a pump on valtron to avoid half-duplex
deadlock).

Duplicating a completion-mode read half is **not** sound. The inbox is keyed by
`Token`, one registration per fd. Two `Connection`s sharing a token would race to
drain the same inbox and each would see a fraction of the stream.

The resolution: **split yields a write-only half.**

```rust
fn split_connection(&self) -> io::Result<Self> {
    match self {
        // …
        Self::Source(s) => s.split_write_half().map(Connection::Source),
    }
}
```

`split_write_half()` `dup(2)`s the fd and returns a source whose `read` is
`Err(BrokenPipe)` and whose `write` is an ordinary `write(2)`. That matches how
the split is actually used (write on the clone, read on the original), and it
fails loudly rather than silently splitting a byte stream in half. Any caller
that reads from a split half is already wrong on TLS too, where
`split_connection` clones the TLS session handle rather than the socket.

This needs confirming against H1's pump before implementation — it is open
question 5.

### The seam, in netio

```rust
// backends/foundation_netio/src/netcap/connection/mod.rs

/// WHY: `Connection` is the one place bytes enter this crate, and the only
/// place a `read(2)` is issued for a plaintext transport. TLS reads *through*
/// it, so a byte source installed here serves cleartext and TLS alike.
///
/// WHAT: an externally supplied, fd-backed, bidirectional byte source.
///
/// HOW: netio never constructs one. A caller that owns a reactor (today only
/// `foundation_http`, via `foundation_nativeapis`) builds it and passes it in.
/// netio stays reactor-agnostic — it only knows how to read and write.
pub trait ConnectionSource: Read + Write + Send + Sync + std::fmt::Debug {
    /// The descriptor behind this source, for readiness registration by the
    /// owner and for `AsRawFd` passthrough.
    #[cfg(unix)]
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd;

    /// Peek without consuming. In completion mode the bytes are already staged,
    /// so this is a slice read rather than a `MSG_PEEK` syscall.
    fn peek(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;

    /// A write-only duplicate of this connection. See `split_connection`: the
    /// read half cannot be duplicated, because the completion inbox is keyed by
    /// a single `Token`.
    ///
    /// # Errors
    /// `Unsupported` if the underlying source cannot be duplicated.
    fn split_write_half(&self) -> std::io::Result<Box<dyn ConnectionSource>>;

    /// Whether `read` here costs no syscall (the kernel already read the bytes).
    ///
    /// Purely observational — a transport's read loop is identical either way,
    /// because `Ok(0)` is EOF and `WouldBlock` means park on both paths. Used
    /// for logging and for the tests that assert the syscall count.
    fn is_kernel_read(&self) -> bool {
        false
    }
}

pub enum Connection {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(unix_net::UnixStream),
    #[cfg(any(feature = "ssl-rustls", feature = "ssl-openssl", feature = "ssl-native-tls"))]
    Tls(TlsStream),

    /// A byte source supplied by the connection's owner. Used by the io_uring
    /// completion read path (Decision 14 F4); nothing in netio builds one.
    Source(Box<dyn ConnectionSource>),
}

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(s) => s.read(buf),
            #[cfg(unix)]
            Self::Unix(s) => s.read(buf),
            #[cfg(any(feature = "ssl-rustls", feature = "ssl-openssl", feature = "ssl-native-tls"))]
            Self::Tls(tls) => tls.read(buf),
            Self::Source(s) => s.read(buf),   // ← inbox pop, zero syscalls
        }
    }
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            // …
            Self::Source(s) => s.write(buf),  // still a write(2); SEND is future work
        }
    }
    // flush likewise
}

#[cfg(unix)]
impl AsRawFd for Connection {
    fn as_raw_fd(&self) -> RawFd {
        match self {
            Self::Tcp(s) => s.as_raw_fd(),
            // …
            Self::Source(s) => s.as_raw_fd(),
        }
    }
}
```

`RawStream::AsPlain(BufferedReader<BufferedWriter<Connection>>, ..)` is
untouched. `RustlsStream<T>(Arc<Mutex<StreamOwned<T, Connection>>>)` is
untouched. `SharedByteBufferStream<RawStream>` is untouched. The WS server task,
the H1 transport, the H2 pump, the `IncrementalDecoder` — all untouched.

### The implementation, in `foundation_nativeapis`

`RegisteredFd::read_bytes` already exists and already does exactly this (feature
43): it copies out of the inbox with no syscall in completion mode, and falls
back to `read(2)` on every other backend. All that is missing is the wrapper.

```rust
// backends/foundation_nativeapis/src/native/fd/connection_source.rs   (new)

/// A `ConnectionSource` whose reads come from the io_uring completion inbox.
///
/// WHY: this is the transport-facing end of Decision 14 F4. A transport that
/// reads through here issues no `read(2)`; the kernel filled the buffer when the
/// data arrived.
///
/// WHAT: `Read` pops the per-token inbox; `Write` is an ordinary `write(2)`
/// (F43 shipped `RECV`, not `SEND`).
///
/// HOW: wraps a `RegisteredFd` registered with `with_completion`. If the fd
/// cannot take the recv path — a non-socket, a listener, or a reactor that is
/// not in completion mode — `read_bytes` transparently issues `read(2)`, so this
/// type is always correct, just not always free.
#[derive(Debug)]
pub struct CompletionSocket<T: AsRawFd + Read + Write> {
    fd: RegisteredFd<T>,
}

impl<T: AsRawFd + Read + Write> CompletionSocket<T> {
    /// # Errors
    /// The selector's registration error.
    pub fn new(inner: T, token: Token) -> io::Result<Self> {
        let reactor = Reactor::get()?;
        let fd = RegisteredFd::with_completion(
            inner,
            reactor.registry(),
            token,
            Interest::READABLE | Interest::WRITABLE,
        )?;
        Ok(Self { fd })
    }

    /// The readiness handle a task parks on: `TaskStatus::Depends(source.readiness())`.
    pub fn readiness(&self) -> Arc<dyn EventReadiness + Send + Sync> { /* … */ }
}

impl<T: AsRawFd + Read + Write> Read for CompletionSocket<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Completion mode: memcpy out of the kernel-filled inbox, no syscall.
        // Otherwise: read(2). Ok(0) is EOF and WouldBlock means park, on both.
        self.fd.read_bytes(buf)
    }
}

impl<T: AsRawFd + Read + Write> Write for CompletionSocket<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> { self.fd.get_mut().write(buf) }
    fn flush(&mut self) -> io::Result<()> { self.fd.get_mut().flush() }
}
```

netio's trait is then implemented **in the crate that owns both** — a blanket
impl in `foundation_http` (or wherever the composition root lands):

```rust
// backends/foundation_http/src/completion_conn.rs   (new)
impl<T> foundation_netio::netcap::ConnectionSource
    for foundation_nativeapis::native::fd::CompletionSocket<T>
where T: AsRawFd + Read + Write + Send + Sync + Debug
{
    fn as_raw_fd(&self) -> RawFd { self.fd.as_raw_fd() }
    fn is_kernel_read(&self) -> bool { self.fd.is_completion_source() }
}
```

Orphan rule: the trait is netio's, the type is nativeapis'. A third crate cannot
implement it. So either the impl lives in nativeapis (needing `netio` as an
optional dep — the reverse edge, which is *safe*: netio is wasm-capable and
nativeapis is not, so nativeapis→netio adds nothing to a wasm build), or the
wrapper type lives in `foundation_http`. **The latter.** `foundation_http`
already depends on both.

### Accepting a connection

`RawStream::from_connection(conn: Connection)` already exists and already does
the `BufferedReader::new(BufferedWriter::new(conn))` wrapping. The accept path
therefore changes by exactly one line — which `Connection` it hands over:

```rust
// in the server accept loop (foundation_http), once per connection
let (tcp, _addr) = listener.accept()?;   // listener stays on POLL_ADD — Constraint 2
tcp.set_nonblocking(true)?;

//  before:  let conn = Connection::from(tcp);
let source = CompletionSocket::new(tcp, Token(next_token()))?;
tracing::debug!(kernel_read = source.is_kernel_read(), "connection accepted");
let conn = Connection::Source(Box::new(source));

let stream = RawStream::from_connection(conn)?;          // unchanged
//        or RawStream::from_server_tls(…)  — rustls wraps that same Connection

let task = WebSocketServerTask::new(SharedByteBufferStream::new(stream), cfg);
```

Nothing downstream changes. `next_status()` still parks on
`Depends(RegisteredFd)`; the decoder still steps over `&buf[..]`.

Note that `from_connection` calls `conn.stream_addr()`, so `Connection::Source`
must answer `peer_addr`/`local_addr` — `getpeername`/`getsockname` on the owned
fd. Cheap, but it is one more arm.

### Why this is the right shape

- **No new dependency edge into netio.** netio stays reactor-agnostic and
  wasm-clean. This was the reason feature 43 stopped short, and it is answered.
- **TLS falls out for free.** rustls is generic over its socket and we already
  instantiate it at `Connection`.
- **Two implementations, one code path.** `read_bytes` is already the unified
  read; `Connection::Source` is already the unified enum arm. There is no
  "completion transport" to keep in sync with the readiness transport.
- **Degrades correctly.** On epoll, on kqueue, on a 5.13 kernel, on a pipe, on a
  listener — `read_bytes` issues `read(2)` and everything works, slower.

---

## Rejected: Option A — `netio → nativeapis` optional dependency

Add `Connection::Completion(CompletionSocket<TcpStream>)` directly, with

```toml
[target.'cfg(all(unix, not(target_arch = "wasm32")))'.dependencies]
foundation_nativeapis = { path = "../foundation_nativeapis", optional = true }

[features]
completion-io = ["dep:foundation_nativeapis"]
```

Concrete, no trait object, no orphan-rule dance, and one less indirection on the
read path.

**Why not.** It puts a native reactor — `libc`, `io-uring`, a `cdylib` — inside
the dependency graph of a crate that compiles to wasm, gated only by a feature
flag that any downstream crate can turn on transitively. netio's whole design is
that readiness comes *from the caller* (`ReadModel::Depends` takes it as an
argument). Inverting that for one backend buys a `Box` and costs the layering.
Worth revisiting only if the `Box<dyn>` shows up in a profile, which it will not
— it is one indirect call per `read`, against a 16 KiB memcpy.

## Rejected: Decision 30's Solution 2 — submit an op per `read()`

Submit `IORING_OP_READ` against the caller's `&mut [u8]`, then `io_uring_enter`
and wait for the single CQE.

**Why not** — and Decision 30 reaches the same conclusion: it is *one syscall per
read*, strictly worse than `libc::read()` for a single call, with the batching win
only appearing when several reads ride one `enter`. It also requires the caller's
buffer to stay put across the `enter`. Multishot `RECV` gets the batching without
either problem: the kernel already holds the buffers, and one SQE covers the fd's
entire lifetime.

## Rejected: Decision 30's Solution 3 — migrate `RawStream` to a trait

`trait Transport: Read + Write + AsRawFd { fn submit_read(..); }`.

**Why not.** Decision 30's own objection is right: the `RawStream` enum is deeply
embedded across `foundation_netio`, `foundation_http` and `foundation_connectrpc`,
and turning it into dynamic dispatch is a separate, larger project. Note that the
recommendation here *does* introduce dynamic dispatch, but at `Connection` and for
one variant only — a `Box<dyn ConnectionSource>` behind an existing enum, not a
migration of the enum itself.

## Rejected: Option C — a parallel "completion transport"

Build `CompletionH1Transport` / `CompletionWsServerTask` alongside the existing
ones.

**Why not.** It doubles every transport, and the two copies drift. Decision 14's
seam analysis is explicit that they should not exist:

> *"valtron's contract — task returns `Depends(source)`, reactor calls
> `wake(token)`, task is re-polled — is byte-blind and identical in both modes;
> it cannot tell a readiness bit from a completed recv. The entire delta lives in
> foundation_nativeapis (ring + buffer lifecycle) and the netio transport tasks
> (byte acquisition)."*

Byte acquisition is one method. A second transport is not a seam, it is a fork.

## Considered and unnecessary: expanding `RawStream` to "fake fds"

`RawStream` needs nothing. Every variant already reaches an fd — that intuition
is correct — but it reaches it *through* `Connection`, which is one layer down
and is where the `read(2)` actually lives. Adding a fourth `RawStream` variant
would:

- duplicate the `Read`/`Write`/`AsRawFd` match arms a fourth time,
- leave the two TLS variants unable to use completion mode (they wrap
  `RustlsStream`, not a socket), and
- give `SharedByteBufferStream<RawStream>` nothing it does not already have.

The `AsRawFd` impl on `RawStream` (`netcap/no_wasm.rs`, TLS variants reach
through to the wrapped TCP socket) already proves the fd is reachable from the
top. What is needed is to change where bytes come *from*, not to re-expose the fd.

---

## Where the win actually shows up: the proxy

Carried forward from Decision 30, whose hot-path analysis is the strongest
argument for doing this at all. `foundation_proxy`'s path is
`ProxyHandler::serve()` → `forward_http()` → `SimpleHttpClient::send()` →
`RawStream::read()`. With `Connection::Source`:

1. **Accept.** The listener stays on `POLL_ADD` — `RECV` on a listening socket is
   meaningless, and F43 already falls back for `SO_ACCEPTCONN` fds.
2. **Read request.** `RawStream::read()` → `Connection::Source::read()` → pop the
   inbox. No syscall. On an empty inbox: `WouldBlock`, and the task parks on
   `Depends`.
3. **Forward upstream.** `SimpleHttpClient` holds its own `RawStream` for the
   backend connection; the same `Connection::Source` applies, so both legs of a
   proxied request read without syscalls.
4. **`splice_bidirectional`.** This is the biggest single win and the clearest
   correctness improvement. It currently spin-polls two nonblocking `Read` handles
   with a **1 ms sleep** between passes. In completion mode both fds are armed
   multishot; the task parks on a composite readiness over the two inboxes and
   wakes when either has bytes. The sleep — and the latency floor it imposes —
   disappears. This works today with the existing seam and does not need the
   zero-copy path.

Note this is *not* zero-copy end to end. The kernel fills a registered buffer;
we `memcpy` it into the caller's slice. Decision 30's phase 4 — both directions
sharing one buffer ring so bytes move fd→buffer→fd without touching user memory —
needs `IORING_OP_SEND` against a provided buffer, which is out of scope here.

## Scope

- `foundation_netio`: `ConnectionSource` trait; `Connection::Source` variant;
  `Read`/`Write`/`AsRawFd`/`AsFd`/`Debug` arms, plus the `PeekableReadStream`,
  `SplitReadStream`, `ReadTimeoutOperations`, and addr/shutdown obligations
  enumerated above. **No new dependency.**
- `foundation_nativeapis`: `CompletionSocket<T>` wrapper over the existing
  `RegisteredFd::with_completion` + `read_bytes`. Small — the machinery is done.
- `foundation_http`: the `ConnectionSource` impl, and an accept path that builds
  `Connection::Source`.
- `foundation_proxy`: `splice_bidirectional` parks on composite readiness instead
  of sleeping 1 ms.
- A `completion-io` runtime switch (env or server-builder option) so an operator
  can force the readiness path without recompiling, and so the parity tests can
  drive both.

## Phased rollout

Adapted from Decision 30, with the phases that F43 already completed struck out.

- ~~**Phase 1: buffer pool + driver.**~~ **Done in F43.** `BufRing` (registered
  `PBUF_RING`, `ProvidedBuf` recycle-on-drop) and the shared `Reactor` drain
  thread already exist and are tested, including pool starvation and recovery.
- **Phase 2: `CompletionSocket` + `Connection::Source`.** The trait, the variant,
  the arms. Gate behind `completion-io`. Read/write parity against `TcpStream`.
- **Phase 3: server + proxy integration.** Accept path builds `Connection::Source`.
  Benchmark proxy throughput with the switch on and off.
- **Phase 4: `splice_bidirectional` without the sleep.** Composite readiness over
  both inboxes.
- **Phase 5 (separate feature): `IORING_OP_SEND` and the zero-copy relay.**
  Out of scope here; needs write-side buffer accounting.

## Verification

- `cargo test -p foundation_netio --features completion-io` — `Connection::Source`
  read/write/peek/split parity against `Connection::Tcp`.
- The ptrace syscall counter from `uring_completion_syscall_tests.rs`, pointed at a
  real served connection rather than a bare socketpair: **zero** read-family
  syscalls on the connection fd, with the epoll-forced run as the control arm.
  (`strace -f -e trace=read,recvfrom,recvmsg` corroborates; `perf stat -e
  raw_syscalls:sys_enter` gives the aggregate Decision 30 asks for.)
- `cargo test -p foundation_proxy --features docker-tests,completion-io`.
- `wrk -c 100 -t 4` against the proxy, io_uring vs epoll. Decision 30 predicts
  2–4× for small requests and 1.2–1.5× for large bodies; treat those as hypotheses
  to test, not targets. The syscall count is the measurement that is not
  hardware-dependent.

## Out of scope

- `IORING_OP_SEND` / write-side completion. Writes stay `write(2)`. Decision 14
  F4 names `RECV`/`SEND` together but F43 shipped only `RECV`; the send path
  wants its own buffer accounting and a separate measurement.
- `accept()` via `IORING_OP_ACCEPT` (multishot accept). Listeners stay on
  `POLL_ADD`.
- HTTP/3 (features 34/35). QUIC reads UDP through its own path.
- Per-worker rings. Re-evaluated and rejected on evidence in D14 OQ#14.1;
  submissions are `O(registrations)`, not `O(reads)`.

## Open questions for review

1. **Where does the `ConnectionSource` impl live?** `foundation_http` is the only
   crate depending on both today, but `foundation_connectrpc` will want it too.
   A small `foundation_netio_uring` glue crate is the alternative. My preference:
   start in `foundation_http`, extract when the second consumer appears.
2. **Token allocation.** The reactor is keyed by `Token(usize)`, and the
   completion selector reserves the top two bits (`POLL_TAG`), so tokens must
   stay below `2^62`. A per-connection monotonic counter is fine; it needs an
   owner and a recycling story for long-lived servers.
3. **`BufferedReader` on top of an inbox.** Completion mode already buffers in
   the kernel-filled `ProvidedBuf`, and `SharedByteBufferStream` buffers again.
   That is one memcpy more than strictly needed. The zero-copy alternative is to
   step the decoder straight over `take_completions()`, which requires the
   transports to speak `Completion` rather than `Read` — a much larger change,
   and the one D14 gestures at with "step over `&buf[..]`". Worth measuring
   before committing to either.
4. **Buffer-pool sizing under many connections.** 256 buffers shared across all
   sockets on one reactor. A slow consumer holding buffers starves the others
   (`-ENOBUFS` → the token parks until a buffer returns; correct, but a
   head-of-line stall). Do we want a pool per reactor, per worker, or per
   listener? This is the question OQ#14.1 deferred, arriving from a different
   direction: not SQ contention, but buffer-pool contention.
5. **Is `split_connection`'s read half ever read?** The proposed
   `split_write_half` is sound only if every caller of `split_connection` writes
   on the clone and reads on the original. H1's pump (feature 23) and the H2
   full-duplex pump (feature 47) both need auditing before this is settled. If a
   caller *does* read from the clone, completion mode needs a different split
   story — most likely one registration serving a shared inbox behind an
   `Arc<Mutex<StagedReads>>`, which changes the ownership model.
6. **Token lifetime vs fd reuse.** A `Token` is dead only after the reactor
   deregisters it. Closing the fd before deregistering leaks an inbox entry and,
   worse, lets a recycled fd number collide with a live registration. The
   `CompletionSocket` `Drop` must deregister before the inner `T` closes the fd —
   `RegisteredFd::drop` already does this, but `Connection::Source` boxing adds a
   drop order to verify.

## Acceptance criteria

- A real WebSocket or HTTP/1.1 connection, served over a TCP socket on a
  completion-capable kernel, completes a request/response with **zero
  read-family syscalls on that socket**, measured by the ptrace counter already
  in `uring_completion_syscall_tests.rs`.
- The same connection over TLS (`ssl-rustls`) does the same, with no
  TLS-specific code.
- With the reactor forced to `BackendPreference::Epoll`, the identical transport
  code path works and *does* issue `read(2)` — the control arm.
- A listening socket still accepts; a pipe-backed transport still reads.
- `cargo check -p foundation_netio --target wasm32-unknown-unknown` is unchanged,
  and netio's dependency graph gains nothing.
