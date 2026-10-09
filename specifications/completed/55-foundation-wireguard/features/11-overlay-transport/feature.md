# Feature 11 — Overlay Transport Bridge (`Connection::Overlay`)

**Depends on:** F04 (data-plane orchestration), F09 (config), F00 (nativeapis dataplane)
**Unblocks:** WireGuard + ConnectRPC integration, WG-mesh HTTP services
**Status:** ✅ Complete (implemented + tested 2026-07-14). All 11 task-list items done: Acceptor/Connector traits, Connection::Overlay variant, OverlayReadWrite impl, WgHandle::overlay_connect, HttpServer::serve_with_acceptor, OverlayAcceptor, TunnelDriver<TunDataPlane> (MeshDataPlane+GossipTransport), example, address bridging, non-blocking WouldBlock parking (BoolSignal+ConnWaker→TaskStatus::Depends wired through request_intro/request_redirect), integration test (POST+GET echo over mesh via NativeHttpClient+OverlayConnector).

## WHY

WireGuard mesh nodes already own a `DataPlane` that hands out `OverlayStream`s
(reliable, ordered byte streams over the smoltcp userspace TCP/IP stack inside
the encrypted tunnel). ConnectRPC already has `H1Transport` that speaks HTTP/1.1
over any `Connection`. The gap is that `Connection` has `Tcp`, `Unix`, `Tls`,
and `Completion` variants — but no `Overlay` variant. Closing that gap means the
entire `foundation_netio` HTTP stack (and therefore ConnectRPC, `foundation_http`,
`HttpServer`, `NativeHttpClient`, gRPC, gRPC-Web) works over the WireGuard mesh
with zero additional transport code.

Without this bridge, every HTTP/RPC consumer that wants to run over the mesh must
write its own smoltcp adapter — exactly the kind of leaky abstraction the
`Connection` enum was designed to prevent.

## WHAT

Six things:

1. **`OverlayReadWrite` trait** — defined in `foundation_netio::native::connection`,
   same pattern as `CompletionReadWrite`: `Read + Write + Send + Sync + Debug + clone_box()`.
   The trait object avoids a `netio → nativeapis → db → netio` crate cycle.

2. **`Connection::Overlay(Box<dyn OverlayReadWrite>)`** — new variant, same shape
   as `Connection::Completion(Box<dyn CompletionReadWrite>)`.

3. **All Connection methods extended** — `Read`, `Write`, `flush`, `AsRawFd` (-1),
   `set_read_timeout`/`set_write_timeout` (no-op), `read_timeout`/`write_timeout` (None),
   `peer_addr`/`local_addr` (None), `shutdown` (Ok), `try_clone`/`split_connection`
   (via `clone_box`), `PeekableReadStream` (`NotSupported`).

4. **`OverlayReadWrite` impl in `foundation_wireguard`** — `OverlayConnection` wraps
   `OverlayStream` (which is `Arc<Mutex<Inner>>` internally), implements
   `Read + Write + OverlayReadWrite`. `clone_box()` is a cheap `Arc` bump
   (`OverlayStream` derives `Clone`).

5. **`WgHandle::overlay_connect(peer_ip, port) -> Connection`** — one call returns
   a full `Connection::Overlay(...)` usable by any netio consumer.

6. **`Acceptor` trait in `foundation_netio`** — abstracts the accept side so
   `HttpServer` can work with kernel TCP listeners AND smoltcp overlay listeners.
   `TcpListener` gets a blanket impl. Wireguard provides `OverlayListener` impl.

## HOW

### Crate dependency graph (no cycles)

```
foundation_netio        →  Defines OverlayReadWrite + Connection::Overlay + Acceptor
       ↑
       │
foundation_wireguard    →  Impl OverlayReadWrite for OverlayStream
                           Impl Acceptor for OverlayListener
                           WgHandle::overlay_connect() → Connection::Overlay(...)
       ↑
       │
foundation_connectrpc   →  Uses H1Transport → Connection — zero code changes
foundation_http         →  Uses HttpServer → Acceptor + Connection — zero code changes
```

The traits live in `foundation_netio` where `Connection` already lives — the
same pattern as `CompletionReadWrite`. The impls live in `foundation_wireguard`
because that's where `OverlayStream` is available.

### `Acceptor` trait (in `foundation_netio`)

```rust
/// Accept incoming connections, returning a `Connection` + socket address.
/// Blanket impl for `TcpListener` for backward compat. Overlay impl in wireguard.
pub trait Acceptor: Send + Sync + 'static {
    type Stream: Read + Write;
    type Addr: Into<SocketAddr>;

    fn accept(&self) -> io::Result<(Self::Stream, Self::Addr)>;
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()>;
    fn local_addr(&self) -> io::Result<SocketAddr>;
}

// Blanket impl — existing TcpListener code works unchanged
impl Acceptor for std::net::TcpListener { ... }
```

### `HttpServer::serve_with_acceptor()`

```rust
impl HttpServer {
    pub fn serve_with_acceptor<A: Acceptor>(
        self,
        acceptor: &A,
        shutdown: &Arc<OnSignal>,
        wrap_stream: impl Fn(A::Stream, SocketAddr) -> Result<RawStream, String>,
    ) { ... }
}
```

### `Connection::Overlay` variant

```rust
pub enum Connection {
    Tcp(TcpStream),
    #[cfg(unix)] Unix(unix_net::UnixStream),
    Tls(TlsStream),
    #[cfg(unix)] Completion(Box<dyn CompletionReadWrite>),
    // NEW:
    Overlay(Box<dyn OverlayReadWrite>),
}
```

### `OverlayReadWrite` trait (in `foundation_netio`)

```rust
pub trait OverlayReadWrite: Read + Write + Debug + Send + Sync {
    fn clone_box(&self) -> Box<dyn OverlayReadWrite>;
}
```

### `OverlayReadWrite` impl (in `foundation_wireguard`)

```rust
struct OverlayConnection {
    stream: OverlayStream,  // Arc<Mutex<Inner>> internally — Clone is cheap
}

impl Read for OverlayConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { self.stream.read(buf) }
}
impl Write for OverlayConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> { self.stream.write(buf) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
impl OverlayReadWrite for OverlayConnection {
    fn clone_box(&self) -> Box<dyn OverlayReadWrite> {
        Box::new(OverlayConnection { stream: self.stream.clone() })
    }
}
```

### Kernel TUN path

When `DataPlaneConfig::Mode = Tun`, the kernel routes `10.x.y.z` through the TUN
device. Apps use `std::net::TcpStream::connect("10.x.y.z:8080")` directly — no
`OverlayReadWrite` needed. The TUN fd is polled by `TunnelDriver<TunDataPlane>`.

### Example — ConnectRPC over WireGuard

```rust
// Two nodes, one mesh
let a = WgNode::from_config(WgConfig::seed(seed, net)).join()?;
let b = WgNode::from_config(WgConfig::joiner(seed, net, vec![a.bootstrap_addr()])).join()?;

// B: bind ConnectRPC server on overlay IP via OverlayAcceptor
let acceptor = OverlayAcceptor::new(b.tcp_listen(8080)?, b.overlay_ip());
let server = HttpServer::with_config(app, "10.0.0.2:8080", defaults);
server.serve_with_acceptor(&acceptor, &shutdown, |stream, addr| {
    RawStream::from_connection(Connection::Overlay(Box::new(stream)))
});

// A: connect to B over the overlay via WgHandle::overlay_connect()
let conn = a.overlay_connect(b.overlay_ip(), 8080)?;
// conn is Connection::Overlay(...) — H1Transport can use this via HttpClient
```

## Non-blocking transport integration (the `WouldBlock` → park model)

**Decision (2026-07-13):** the overlay is the **first non-blocking client transport**.
Every prior `Connection` on the client read path was a blocking `TcpStream`
(`SO_RCVTIMEO`) that never returns `WouldBlock` mid-read — it blocks until data or
timeout. `OverlayStream` (smoltcp) is non-blocking: `read` returns
`io::ErrorKind::WouldBlock` the instant no data is buffered. The netio client
reader treated any read error — including `WouldBlock` — as a **fatal**
`LineReadFailed` and drove the task to `Ready(Failed)`. So an overlay HTTP request
failed the moment it tried to read the response before the first byte arrived.

Two integration models were weighed:

- **Blocking adapter** — spin+sleep inside `OverlayConnection::read` until data or a
  read-timeout, presenting blocking semantics. Small and contained (~30 lines), but
  blocks a valtron worker per read (the `TunnelDriver` runs on its own OS thread, so
  packets still flow, but a pool worker is tied up).
- **Non-blocking parking (CHOSEN)** — the reader treats `WouldBlock` as *not ready*
  (not a failure) and the task **parks** on a valtron `TaskStatus::Depends(signal)`,
  unparked by the overlay's read/write waker fired by the `TunnelDriver` when the
  smoltcp socket becomes readable/writable. This is the same `Depends` model
  Completion/io_uring mode uses, and it generalises to *any* future non-blocking
  client transport. No worker blocks.

### Retry-safety (why parking is sound)

`SharedByteBufferStream::read_line` peeks via `next_until` and only commits with
`skip()` **after** a full line — on `WouldBlock` **no bytes are consumed**, so a
retry resumes exactly where it left off. The header block is made atomic the same
way: `peek_until(b"\r\n\r\n")` confirms the whole head is buffered *before*
`parse_headers` consumes any line, so a partial multi-packet head parks without
losing consumed lines.

### Mechanism

1. **`HttpReaderError::WouldBlock`** — new variant. `HttpResponseReader::next` maps a
   `WouldBlock` io error to it and returns `Some(Err(WouldBlock))` **without**
   advancing `state` to `Finished` (Intro stays Intro; Headers stays Headers, guarded
   by `peek_until` so parsing is atomic).
2. **Waker plumbing** — `OverlayReadWrite` gains `set_read_waker`/`set_write_waker`;
   `Connection::register_read_waker(&self, WakeFn) -> bool` (Overlay delegates, other
   transports return `false`); `RawStream::register_read_waker` exposes it to tasks.
3. **Task parking** — `request_intro`, `request_redirect`, and the body reader catch
   `HttpReaderError::WouldBlock`: build a `BoolSignal`, register a `WakeFn` that flips
   it via `set_read_waker`, re-store their state, and return
   `TaskStatus::Depends(BoolSignal)`. One extra read is retried after registration to
   close the register-vs-arrival race (avoids a lost wakeup).
4. **Server side** — `HttpServer`'s serve loop applies the same `WouldBlock` → retry
   treatment reading the request off an overlay `Connection`.

### Address bridging (fixed 2026-07-13)

`RawStream::from_connection` calls `Connection::stream_addr` which needs a concrete
local+peer address; smoltcp has no `getsockname(2)`, so `OverlayStream` carries
`local`/`peer` captured at connect/accept time, surfaced through
`OverlayReadWrite::{local_addr,peer_addr}` → `Connection::Overlay`. Without it both
sides failed with `FailedToAcquireAddrs`.

## Task list

1. **Acceptor trait** — in `foundation_netio::native::connection`, blank impl for `TcpListener`. **DONE**
2. **Connection::Overlay** variant + all match arms. **DONE**
3. **OverlayReadWrite impl** in wireguard — `OverlayConnection` wrapping `OverlayStream`. **DONE**
4. **WgHandle::overlay_connect()** — opens overlay TCP, drives handshake to established, returns `Connection::Overlay(...)`. **DONE**
5. **HttpServer::serve_with_acceptor()** — generic over `Acceptor`, backward-compat with `serve_with_listener`. **DONE**
6. **OverlayAcceptor** in wireguard — wraps `OverlayListener`, implements `Acceptor`. **DONE**
7. **TunnelDriver<TunDataPlane>** — kernel TUN path in `WgNode::join()`. **DONE** (2026-07-14: `MeshDataPlane` enum + `GossipTransport` + optional `NetStack` in `WgHandle`)
8. **Example** — two-node mesh + HTTP over overlay via `overlay_connect`. **DONE**
9. **Address bridging** — `OverlayReadWrite::{local_addr,peer_addr}` → `Connection::Overlay`. **DONE**
10. **Non-blocking parking** — `HttpReaderError::WouldBlock` + waker plumbing + task/server parking. **IN PROGRESS**
11. **Tests** — `overlay_connector_http_round_trip` end-to-end (`foundation_http/tests/overlay_integration_tests.rs`); WouldBlock park/retry; Acceptor accept.

## Test plan

- **Unit**: `OverlayConnection` read/write round-trips through an `OverlayStream` bridged by an in-memory packet queue.
- **Integration**: Two-node WireGuard mesh + HTTP request/response over `Connection::Overlay`. A sends `GET /`, B responds `HTTP/1.1 200`, A reads the response. Verifies end-to-end: WG handshake → smoltcp TCP → raw HTTP bytes.
- **Acceptor**: `OverlayAcceptor` accept loop delivers `OverlayStream` connections. `TcpListener` blanket impl works identically.

## Related decisions

- [02 — Dual data plane (smoltcp + TUN)](../../decisions/02-dual-dataplane.md)
- [14 — Crate layering](../../decisions/14-crate-layering.md)
- Feature [04 — Mesh & data-plane orchestration](../04-mesh-and-dataplane-orchestration/feature.md)
- `foundation_iogate` — `CompletionReadWrite` trait-object pattern (the reference for this design)
