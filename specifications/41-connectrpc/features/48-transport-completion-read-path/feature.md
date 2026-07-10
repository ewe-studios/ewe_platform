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

Not the types, and — on review — **not the dependency direction either**.

The first draft of this document treated `netio → nativeapis` as the blocker,
because `foundation_netio` compiles to wasm and `foundation_nativeapis` is native
only (`crate-type = ["rlib", "cdylib"]`, `libc`, `io-uring`). That reasoning was
wrong twice over:

- netio **already** has a `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`
  table, and already splits `netcap/no_wasm.rs` from `netcap/wasm.rs`. A native-only
  dependency in a native-only module is the established pattern here, not a new one.
- The house rule is explicit: *target-gate native tooling, never feature-gate it* —
  "the wasm/native split is a target property, not a configuration choice". A
  `completion-io` feature flag that any downstream crate could flip on transitively
  would be exactly the friction that rule exists to prevent.
- `foundation_nativeapis` does not depend on `foundation_netio`, so the edge is
  acyclic.

So netio takes a **target-gated, unconditional** dependency on nativeapis, the
completion `Connection` variant lives in netio's native-only module, and there is no
feature flag, no `Box<dyn>`, and no orphan-rule dance.

What *does* block it is the **registration lifecycle**, and one defect underneath it:

- Someone must register the fd with the shared reactor, choose the completion path,
  and deregister on close. Today netio's `ReadModel::Depends` receives readiness
  *from the caller* precisely so netio owns none of that.
- **`Reactor::get()` ignores backend preference entirely.** F42 built
  `Poll::with_preference` and the "explicit backend is a requirement, probe failure
  is a hard error" semantics of D14 OQ#14.3 — and then `Reactor::get()`, a `OnceLock`
  singleton, calls `Poll::new()` (i.e. `BackendPreference::Auto`) unconditionally.
  The shared reactor is the only thing any transport touches. So today an operator
  cannot demand io_uring, cannot demand epoll, and cannot discover that the request
  was ignored. The no-silent-defaults rule is defeated at the one place it matters.
  Fixing that is a **prerequisite** of this feature, not a nicety — see
  *Explicit I/O backend selection* below.

## Prerequisite: netio shared/native/wasm restructuring

`foundation_netio` already uses `shared`/`native`/`wasm` subdirectories inside
`event_source`, `simple_http`, and `websocket`. The top-level module layout and
`netcap` still use the older `no_wasm.rs` / `wasm.rs` sibling-file pattern. Before
adding a native-only `Connection` variant that depends on `foundation_nativeapis`,
the crate is reorganised to match `foundation_http`'s top-level structure:

```
src/
├── lib.rs                  // #[cfg] gates on the module declarations only
├── shared/                 // always compiled
│   ├── mod.rs
│   ├── errors.rs           // from netcap/errors/
│   ├── core.rs             // from netcap/core.rs (DataStream, IntoHeaders)
│   ├── context.rs          // from netcap/context.rs
│   └── endpoint.rs         // Endpoint, EndpointConfig, SocketAddr
├── native/                 // #[cfg(not(target_family = "wasm"))]
│   ├── mod.rs
│   ├── connection.rs       // from netcap/connection/ — the Connection enum
│   ├── raw_stream.rs       // from netcap/no_wasm.rs — RawStream
│   ├── ssl/                // from netcap/ssl/
│   ├── http2/              // from http2/
│   ├── http3/              // from http3/ (behind quic feature)
│   ├── quic/               // from quic/ (behind quic feature)
│   ├── http_stream/        // from http_stream/
│   ├── completion.rs       // ← NEW: Connection::Completion arm + CompletionSocket import
│   └── [event_source|websocket|simple_http]/native/
└── wasm/                   // #[cfg(target_family = "wasm")]
    ├── mod.rs
    ├── netcap.rs           // from netcap/wasm.rs
    └── simple_http/wasm/
```

After this, a file in `src/native/` needs **no per-item `#[cfg]` gates** — the
module declaration in `lib.rs` already carries the gate. The `Connection` enum
lives in `native/connection.rs`, so a new `Completion(CompletionSocket<TcpStream>)`
variant is just another variant in a native-only file, with zero conditional
compilation noise.

The `event_source`, `simple_http`, and `websocket` subtrees keep their internal
`shared`/`native`/`wasm` split; they move under the top-level directories of the
same name. Re-exports from `lib.rs` preserve the existing public API so no
downstream crate breaks.

This is not a new pattern. `foundation_http/src/lib.rs` already reads:

```rust
#[cfg(not(target_family = "wasm"))]
pub mod native;

#[cfg(any(target_family = "wasm", feature = "wasm-test"))]
pub mod wasm;
```

and every file under `native/` is implicitly native-only. The restructuring is
**phase 0** of this feature — it unblocks the `Completion` variant by making the
right place for it exist before the feature touches any logic.

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

## Recommendation: a `Connection::Completion` variant in netio's `native/`

After the restructuring, `Connection` lives in `src/native/connection.rs` — a file
that is already native-only by virtue of the module declaration in `lib.rs`. So the
new variant is just **another variant in a native-only file**: no `#[cfg]`, no
feature flag, no conditional compilation noise.

netio's existing `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` table
gains one line:

```toml
foundation_nativeapis = { path = "../foundation_nativeapis", features = ["fd"] }
```

`CompletionSocket` is a concrete type from that crate. No trait, no orphan rule, no
`Box<dyn>`.

### What `Connection` actually demands

A new variant is not free. `Connection` carries more than `Read + Write`, and
every one of these needs a `Completion` arm. This is the real cost of the feature,
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
| `SplitReadStream::split_connection` | **Returns `Err(Unsupported)`.** See below. |
| `try_clone` | **Returns `Err(Unsupported)`.** Same reason. |

#### `split_connection` and `try_clone` become explicit errors

`SplitReadStream::split_connection` has **zero callers**. `grep -rn "split_connection"`
matches only its own trait definition in `foundation_core` and the `Connection` impl.
`Connection::try_clone` is likewise unused outside netio. Both are API surface nobody
exercises, and both are unsound on a completion socket for the same reason: the inbox
is keyed by one `Token`, one registration per fd, so two `Connection`s sharing it
would race to drain the same inbox and each would see a fraction of the byte stream.

Rather than invent a `split_write_half()` that silently returns a crippled handle,
both fail loudly on the completion path:

```rust
impl SplitReadStream for Connection {
    fn split_connection(&self) -> io::Result<Self> {
        match self {
            Self::Tcp(inner) => inner.try_clone().map(Connection::Tcp),
            #[cfg(unix)]
            Self::Unix(inner) => inner.try_clone().map(Connection::Unix),
            #[cfg(any(feature = "ssl-rustls", feature = "ssl-openssl", feature = "ssl-native-tls"))]
            Self::Tls(inner) => inner.try_clone().map(Connection::Tls),
            Self::Completion(_) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "a completion socket cannot be split by cloning: its inbox is keyed \
                 by one Token. Use ReadWriteStream::split_read_write.",
            )),
        }
    }
}
```

and a new, **explicit** trait says what the caller actually wants:

```rust
/// WHY: `split_connection` is a `try_clone` in disguise — it duplicates the fd and
/// hands back a second full-duplex handle, leaving "who reads?" to convention. That
/// is unsound for a completion socket and was never what any caller wanted: H1's
/// pump writes on one half and reads on the other.
///
/// WHAT: an honest asymmetric split into a read half and a write half.
///
/// HOW: the read half keeps the reactor registration and therefore the inbox. The
/// write half `dup(2)`s the fd and writes with `write(2)`.
pub trait ReadWriteStream: Sized {
    type ReadHalf: Read;
    type WriteHalf: Write;

    /// # Errors
    /// `Unsupported` if the underlying stream cannot be split.
    fn split_read_write(self) -> io::Result<(Self::ReadHalf, Self::WriteHalf)>;
}
```

This is strictly better than the status quo even ignoring io_uring: today a caller
that reads from *both* halves of a `split_connection` gets silently interleaved
garbage, and nothing in the type system objects.

### The seam, in netio

After the restructuring, `Connection` lives in `src/native/connection.rs` —
the module is already gated native-only. The existing
`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` table gains one line:

```toml
foundation_nativeapis = { path = "../foundation_nativeapis", features = ["fd"] }
```

and the enum gains one variant, no `#[cfg]` needed:

```rust
// backends/foundation_netio/src/native/connection.rs

pub enum Connection {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(unix_net::UnixStream),
    #[cfg(any(feature = "ssl-rustls", feature = "ssl-openssl", feature = "ssl-native-tls"))]
    Tls(TlsStream),

    /// A socket whose reads come from the io_uring completion inbox — the kernel
    /// already performed them (Decision 14 F4).
    ///
    /// This file is native-only (gated in lib.rs), so no per-variant cfg.
    Completion(CompletionSocket<TcpStream>),
}

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(s) => s.read(buf),
            #[cfg(unix)]
            Self::Unix(s) => s.read(buf),
            #[cfg(any(feature = "ssl-rustls", feature = "ssl-openssl", feature = "ssl-native-tls"))]
            Self::Tls(tls) => tls.read(buf),
            Self::Completion(s) => s.read(buf),   // ← inbox pop, zero syscalls
        }
    }
}
```

`RawStream::AsPlain(BufferedReader<BufferedWriter<Connection>>, ..)` is untouched.
`RustlsStream<T>(Arc<Mutex<StreamOwned<T, Connection>>>)` is untouched.
`SharedByteBufferStream<RawStream>` is untouched. The WS server task, the H1
transport, the H2 pump, the `IncrementalDecoder` — all untouched.

### The implementation, in `foundation_nativeapis`

`RegisteredFd::read_bytes` already exists and already does exactly this (feature
43): it copies out of the inbox with no syscall in completion mode, and falls back
to `read(2)` on every other backend. All that is missing is the wrapper — and it
lives in nativeapis, so netio imports a concrete type rather than defining a trait
for one implementor.

```rust
// backends/foundation_nativeapis/src/native/fd/completion_socket.rs   (new)

/// A socket whose reads come from the io_uring completion inbox.
///
/// WHY: the transport-facing end of Decision 14 F4. A transport that reads through
/// here issues no `read(2)`; the kernel filled the buffer when the data arrived.
///
/// WHAT: `Read` pops the per-token inbox; `Write` is an ordinary `write(2)` (F43
/// shipped `RECV`, not `SEND`).
///
/// HOW: wraps a `RegisteredFd` registered with `with_completion`. If the fd cannot
/// take the recv path — a non-socket, a listener, or a reactor that is not in
/// completion mode — `read_bytes` transparently issues `read(2)`, so this type is
/// always correct, just not always free.
#[derive(Debug)]
pub struct CompletionSocket<T: AsRawFd + Read + Write> {
    fd: RegisteredFd<T>,
    /// The accepted peer address, captured at `accept()` rather than recovered with
    /// a `getpeername(2)` on every `peer_addr()` call.
    peer: Option<SocketAddr>,
}

impl<T: AsRawFd + Read + Write> CompletionSocket<T> {
    /// # Errors
    /// The selector's registration error.
    pub fn new(inner: T, token: Token, peer: Option<SocketAddr>) -> io::Result<Self> {
        let reactor = Reactor::get()?;
        let fd = RegisteredFd::with_completion(
            inner,
            reactor.registry(),
            token,
            Interest::READABLE | Interest::WRITABLE,
        )?;
        Ok(Self { fd, peer })
    }

    /// The readiness handle a task parks on: `TaskStatus::Depends(source.readiness())`.
    pub fn readiness(&self) -> EventReadinessPtr { /* … */ }

    /// Whether `read` here costs no syscall. Observational: a transport's read loop
    /// is identical either way, because `Ok(0)` is EOF and `WouldBlock` means park.
    pub fn is_kernel_read(&self) -> bool { self.fd.is_completion_source() }

    /// The peer address captured at accept time.
    pub fn peer_addr(&self) -> Option<SocketAddr> { self.peer }
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

No trait, no orphan rule, no boxing. netio names the type directly, in a file
that is already native-only.

### Explicit I/O backend selection (server-facing)

**An operator must be able to say which I/O path a server uses, and be told when
that is impossible.** Three modes, and they are genuinely different:

| Mode | Registration | Read | Park |
|---|---|---|---|
| `ServerIo::Std` | none | `read(2)` on the socket | blocking / `WouldBlock` retry — today's behaviour |
| `ServerIo::Readiness` | shared reactor | `read(2)` on the socket | `Depends(RegisteredFd)` — epoll, or io_uring multishot poll |
| `ServerIo::Completion` | shared reactor, `register_recv_fd` | pop the inbox, **no syscall** | `Depends(RegisteredFd)` |
| `ServerIo::Auto` | shared reactor | best available | surfaced in a log line |

```rust
HttpServer::bind(addr)
    .with_io(ServerIo::Completion)   // hard error if the kernel cannot
    .serve(app)?;
```

The same knob belongs on the TCP and UDP servers, for the same reason: keeping
`ServerIo::Std` as the default means **nothing changes for anyone** who does not ask,
and the existing structure and its plain kernel calls stay exactly as they are.

#### The prerequisite: `Reactor::get()` must honour a preference

This cannot be built on today's reactor. `Reactor::get()` is a `OnceLock` singleton
that calls `Poll::new()` — `BackendPreference::Auto` — and F42's `Poll::with_preference`
is never reached from it. So `ServerIo::Completion` has nothing to ask.

The reactor is **process-global** by design (D14 OQ#14.1), so the backend is a
process-level decision that the first initialiser wins. That makes the honest API:

```rust
impl Reactor {
    /// Initialise the shared reactor on a specific backend. First call wins.
    ///
    /// # Errors
    /// - the probe's concrete failure, if `preference` is unavailable
    ///   (`BackendPreference::Uring` on a host with `kernel.io_uring_disabled=2`);
    /// - `AlreadyInitialised { running: Backend, requested: BackendPreference }` if
    ///   the reactor is already up on a *different* backend.
    pub fn init(preference: BackendPreference) -> io::Result<Arc<Self>>;

    /// The shared reactor, initialising it with `Auto` if nobody has yet.
    pub fn get() -> io::Result<Arc<Self>>;
}
```

The `AlreadyInitialised` error is the point. Two servers in one process asking for
different backends is not something we can honour, and silently giving the second one
the first one's backend is precisely the failure D14 OQ#14.3 calls "the worst" — a
perf-critical deploy discovering from latency graphs that it never got io_uring. It
must fail at startup, naming both backends.

`ServerIo::Std` never touches the reactor, so it never conflicts.

### Accepting a connection

`RawStream::from_connection(conn: Connection)` already exists and already does the
`BufferedReader::new(BufferedWriter::new(conn))` wrapping. The accept path changes by
one line — which `Connection` it hands over — and it hands the peer address over too:

```rust
// in the server accept loop, once per connection
let (tcp, peer) = listener.accept()?;   // listener stays on POLL_ADD — Constraint 2
tcp.set_nonblocking(true)?;

let conn = match io_mode {
    // Unchanged: no reactor, no registration, plain read(2).
    ServerIo::Std => Connection::from(tcp),

    // The kernel reads for us. `peer` is carried into the socket rather than
    // recovered with a `getpeername(2)` on every `peer_addr()` call — we already
    // know it, and a completion socket has no other cheap way to answer.
    ServerIo::Completion | ServerIo::Auto | ServerIo::Readiness => {
        let sock = CompletionSocket::new(tcp, Token(next_token()), Some(peer))?;
        tracing::debug!(kernel_read = sock.is_kernel_read(), %peer, "connection accepted");
        Connection::Completion(sock)
    }
};

let stream = RawStream::from_connection(conn)?;          // unchanged
//        or RawStream::from_server_tls(…)  — rustls wraps that same Connection
```

`CompletionSocket` is correct in all three reactor modes: `read_bytes` falls back to
`read(2)` whenever the fd did not take the recv path. `ServerIo::Readiness` therefore
differs from `ServerIo::Completion` only in whether `register_recv_fd` armed a RECV —
which is exactly the per-registration opt-in F43 landed.

Note that `from_connection` calls `conn.stream_addr()`, so `Connection::Completion`
must answer `peer_addr`/`local_addr`. `peer_addr` comes from the accept; `local_addr`
is a `getsockname(2)` on the owned fd, once.

### Why this is the right shape

- **The restructuring does the heavy lifting.** Once `Connection` is in
  `src/native/`, it has access to every native-only dependency netio already
  carries. `foundation_nativeapis` is just another entry in the existing
  target-gated table — not a new edge, not a new pattern.
- **TLS falls out for free.** rustls is generic over its socket and we already
  instantiate it at `Connection`.
- **Two implementations, one code path.** `read_bytes` is already the unified
  read; `Connection::Completion` is just another enum arm. There is no
  "completion transport" to keep in sync with the readiness transport.
- **Degrades correctly.** On epoll, on kqueue, on a 5.13 kernel, on a pipe, on a
  listener — `read_bytes` issues `read(2)` and everything works, slower.
- **The wasm build never sees it.** The module gate in `lib.rs` means
  `cargo check -p foundation_netio --target wasm32-unknown-unknown` compiles
  `shared/` only, exactly as it does now.

---

## Rejected: a `completion-io` feature flag

The approach that ships is a target-gated, unconditional dependency. An
alternative is to make it optional behind a feature flag:

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
foundation_nativeapis = { path = "../foundation_nativeapis", optional = true }

[features]
completion-io = ["dep:foundation_nativeapis"]
```

**Why not.** The target gate already keeps nativeapis out of the wasm build. The
feature flag is therefore a second, redundant gate on top of the first — and it
is the wrong kind. A feature flag is a build-time choice that any downstream crate
can flip on or off transitively, producing two different netios from the same
source. The house rule is explicit: *target-gate native tooling, never
feature-gate it* — "the wasm/native split is a target property, not a
configuration choice."

The restructuring makes this even clearer: `src/native/` is native-only by
construction. Adding a feature flag on top of the module gate would mean a
downstream crate could enable `completion-io` and then... still not get the
`Completion` variant on wasm, because the module is gated. The flag would control
*nothing* on wasm and be *mandatory* on native to use a variant in a module that
is already compiled. That is pure friction with no safety value.

There is only one netio on Linux. The `ServerIo` enum (Std/Readiness/Completion)
is the runtime switch, and it already covers every use case the feature flag
would: an operator who wants io_uring asks for it, and the server errors at
startup if the kernel cannot deliver.

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
and turning it into dynamic dispatch is a separate, larger project. The
recommendation here adds a concrete variant to `Connection` — no dynamic dispatch,
no trait migration — and everything above it is unchanged.

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
`RawStream::read()`. With `Connection::Completion`:

1. **Accept.** The listener stays on `POLL_ADD` — `RECV` on a listening socket is
   meaningless, and F43 already falls back for `SO_ACCEPTCONN` fds.
2. **Read request.** `RawStream::read()` → `Connection::Completion::read()` → pop the
   inbox. No syscall. On an empty inbox: `WouldBlock`, and the task parks on
   `Depends`.
3. **Forward upstream.** `SimpleHttpClient` holds its own `RawStream` for the
   backend connection; the same `Connection::Completion` applies, so both legs of a
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

- **Phase 0 — `foundation_netio` restructuring.** Reorganise the crate into
  top-level `shared/`, `native/`, `wasm/` directories, matching
  `foundation_http`. The module declarations in `lib.rs` carry the `#[cfg]` gates;
  files within each directory are implicitly platform-scoped. Existing public API
  is preserved through re-exports. This is a prerequisite — it creates the
  `src/native/` directory where the `Completion` variant will live without
  per-item conditional compilation.
- `foundation_netio` (post-restructuring): `Connection::Completion` variant in
  `src/native/connection.rs`; `Read`/`Write`/`AsRawFd`/`AsFd`/`Debug` arms, plus
  the `PeekableReadStream`, `SplitReadStream` (returns `Err(Unsupported)`),
  `ReadTimeoutOperations` (documented no-op), `ReadWriteStream` trait, and
  addr/shutdown obligations enumerated above. A target-gated dep on
  `foundation_nativeapis` (fd feature) in the existing
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` table.
- `foundation_nativeapis`: `CompletionSocket<T>` wrapper over the existing
  `RegisteredFd::with_completion` + `read_bytes`. Small — the machinery is done.
- `Reactor::init(preference)` — the prerequisite fix so the shared reactor
  honours an explicit backend preference rather than silently choosing `Auto`.
- `foundation_http`: `ServerIo` enum (Std/Readiness/Completion/Auto) on the
  server builder; accept path that builds `Connection::Completion` when the
  server is not in `Std` mode.
- `foundation_proxy`: `splice_bidirectional` parks on composite readiness instead
  of sleeping 1 ms.

## Phased rollout

- **Phase 0: netio shared/native/wasm restructuring.** Move files into
  top-level `shared/`, `native/`, `wasm/` directories with module gates in
  `lib.rs`. Preserve the existing public API. Verify `cargo check` on both
  native and `wasm32-unknown-unknown`. This phase carries no functional change.
- ~~**Phase 1: buffer pool + driver.**~~ **Done in F43.** `BufRing` (registered
  `PBUF_RING`, `ProvidedBuf` recycle-on-drop) and the shared `Reactor` drain
  thread already exist and are tested, including pool starvation and recovery.
- **Phase 2: `Reactor::init` + `CompletionSocket` + `Connection::Completion`.**
  Add `Reactor::init(preference)` so the shared reactor can be started on a
  specific backend. Add `CompletionSocket<T>` in `foundation_nativeapis`.
  Add the `Completion` variant to `Connection` (in `src/native/` — no per-item
  `#[cfg]`) with all the trait arms enumerated above. Read/write parity against
  `Connection::Tcp`.
- **Phase 3: `ServerIo` + server + proxy integration.** `ServerIo` enum on the
  server builder. Accept path builds `Connection::Completion` when not `Std`.
  Benchmark proxy throughput with the switch on and off.
- **Phase 4: `splice_bidirectional` without the sleep.** Composite readiness over
  both inboxes.
- **Phase 5 (separate feature): `IORING_OP_SEND` and the zero-copy relay.**
  Scoped as **[Feature 49](../49-write-side-completion/feature.md)**; needs
  write-side buffer accounting and a user-filled SEND pool.

## Verification

- `cargo test -p foundation_netio` — `Connection::Completion`
  read/write/peek/split parity against `Connection::Tcp`. No feature flag needed;
  the tests run on any native target.
- `cargo check -p foundation_netio --target wasm32-unknown-unknown` — the wasm
  build sees `shared/` only, exactly as before the restructuring.
- The ptrace syscall counter from `uring_completion_syscall_tests.rs`, pointed at a
  real served connection rather than a bare socketpair: **zero** read-family
  syscalls on the connection fd, with the epoll-forced run as the control arm.
  (`strace -f -e trace=read,recvfrom,recvmsg` corroborates; `perf stat -e
  raw_syscalls:sys_enter` gives the aggregate Decision 30 asks for.)
- `cargo test -p foundation_proxy --features docker-tests`.
- `wrk -c 100 -t 4` against the proxy, io_uring vs epoll. Decision 30 predicts
  2–4× for small requests and 1.2–1.5× for large bodies; treat those as hypotheses
  to test, not targets. The syscall count is the measurement that is not
  hardware-dependent.

## Out of scope

- `IORING_OP_SEND` / write-side completion — deferred to
  **[Feature 49](../49-write-side-completion/feature.md)**. See that document for
  the buffer-ownership model, the `SendPool` design, the `SendSource` trait, and
  the phased rollout (SendPool → measurement → completion-backed write half →
  zero-copy relay).
- `accept()` via `IORING_OP_ACCEPT` (multishot accept). Listeners stay on
  `POLL_ADD`.
- HTTP/3 (features 34/35). QUIC reads UDP through its own path.
- Per-worker rings. Re-evaluated and rejected on evidence in D14 OQ#14.1;
  submissions are `O(registrations)`, not `O(reads)`.

## Open questions for review

1. **Token allocation.** The reactor is keyed by `Token(usize)`, and the
   completion selector reserves the top two bits (`POLL_TAG`), so tokens must
   stay below `2^62`. A per-connection monotonic counter is fine; it needs an
   owner and a recycling story for long-lived servers. After the restructuring,
   the counter naturally lives in `src/native/` — but *which* module owns it
   (a `Connection` constructor? the accept loop?) is open.
2. **`BufferedReader` on top of an inbox.** Completion mode already buffers in
   the kernel-filled `ProvidedBuf`, and `SharedByteBufferStream` buffers again.
   That is one memcpy more than strictly needed. The zero-copy alternative is to
   step the decoder straight over `take_completions()`, which requires the
   transports to speak `Completion` rather than `Read` — a much larger change,
   and the one D14 gestures at with "step over `&buf[..]`". Worth measuring
   before committing to either.
3. **Buffer-pool sizing under many connections.** 256 buffers shared across all
   sockets on one reactor. A slow consumer holding buffers starves the others
   (`-ENOBUFS` → the token parks until a buffer returns; correct, but a
   head-of-line stall). Do we want a pool per reactor, per worker, or per
   listener? This is the question OQ#14.1 deferred, arriving from a different
   direction: not SQ contention, but buffer-pool contention.
4. **Is `split_connection`'s read half ever read?** The proposed
   `ReadWriteStream::split_read_write` is sound only if every caller of
   `split_connection` writes on the clone and reads on the original. H1's pump
   (feature 23) and the H2 full-duplex pump (feature 47) both need auditing before
   this is settled. If a caller *does* read from the clone, completion mode needs a
   different split story — most likely one registration serving a shared inbox
   behind an `Arc<Mutex<StagedReads>>`, which changes the ownership model.
5. **Token lifetime vs fd reuse.** A `Token` is dead only after the reactor
   deregisters it. Closing the fd before deregistering leaks an inbox entry and,
   worse, lets a recycled fd number collide with a live registration. The
   `CompletionSocket` `Drop` must deregister before the inner `T` closes the fd —
   `RegisteredFd::drop` already does this. Verify that `Connection::Completion`
   drops `CompletionSocket` before the inner `TcpStream` — the enum variant field
   order guarantees this, but it belongs in a test, not a comment.

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
- `cargo check -p foundation_netio --target wasm32-unknown-unknown` is unchanged —
  the wasm build compiles `shared/` only, exactly as it does before the restructuring.
- `cargo check -p foundation_netio` on native picks up `foundation_nativeapis` as a
  target-gated dependency in the existing table — no new feature flag, no new pattern.
