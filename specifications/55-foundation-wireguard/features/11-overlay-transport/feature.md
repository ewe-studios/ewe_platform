# Feature 11 — Overlay Transport Bridge (`Connection::Overlay`)

**Depends on:** F04 (data-plane orchestration), F09 (config), F00 (nativeapis dataplane)
**Unblocks:** WireGuard + ConnectRPC integration, WG-mesh HTTP services

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

## Task list

1. **Acceptor trait** — in `foundation_netio::native::connection`, blank impl for `TcpListener`. **DONE**
2. **Connection::Overlay** variant + all match arms. **DONE**
3. **OverlayReadWrite impl** in wireguard — `OverlayConnection` wrapping `OverlayStream`. **DONE**
4. **WgHandle::overlay_connect()** — opens overlay TCP, returns `Connection::Overlay(...)`. **DONE**
5. **HttpServer::serve_with_acceptor()** — generic over `Acceptor`, backward-compat with `serve_with_listener`. **TODO**
6. **OverlayAcceptor** in wireguard — wraps `OverlayListener`, implements `Acceptor`. **TODO**
7. **TunnelDriver<TunDataPlane>** — kernel TUN path in `WgNode::join()`. **TODO**
8. **Example** — two-node mesh + HTTP over overlay via `overlay_connect`. **DONE**
9. **Tests** — `Connection::Overlay` round-trip; WouldBlock retry; Acceptor accept; overlay acceptor.

## Test plan

- **Unit**: `OverlayConnection` read/write round-trips through an `OverlayStream` bridged by an in-memory packet queue.
- **Integration**: Two-node WireGuard mesh + HTTP request/response over `Connection::Overlay`. A sends `GET /`, B responds `HTTP/1.1 200`, A reads the response. Verifies end-to-end: WG handshake → smoltcp TCP → raw HTTP bytes.
- **Acceptor**: `OverlayAcceptor` accept loop delivers `OverlayStream` connections. `TcpListener` blanket impl works identically.

## Related decisions

- [02 — Dual data plane (smoltcp + TUN)](../../decisions/02-dual-dataplane.md)
- [14 — Crate layering](../../decisions/14-crate-layering.md)
- Feature [04 — Mesh & data-plane orchestration](../04-mesh-and-dataplane-orchestration/feature.md)
- `foundation_iogate` — `CompletionReadWrite` trait-object pattern (the reference for this design)
