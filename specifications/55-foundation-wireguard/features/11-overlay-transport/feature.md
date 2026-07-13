# Feature 11 — Overlay Transport Bridge (`Connection::Overlay`)

**Depends on:** F04 (data-plane orchestration), F09 (config), F06 (netio quic)
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

1. **`OverlayReadWrite` trait** — in `foundation_netio::native::connection`,
   same pattern as `CompletionReadWrite`: `Read + Write + Send + Sync + Debug`.
   The trait object avoids a `netio → nativeapis → db → netio` cycle.
2. **`Connection::Overlay(Box<dyn OverlayReadWrite>)`** — new variant delegating
   `Read`/`Write`/`AsRawFd`/`ReadTimeoutOperations` to the boxed trait.
3. **`OverlayReadWrite` impl in `foundation_wireguard`** — wraps `OverlayStream`
   (which locks an internal `Arc<Mutex<>>` so `&self` methods work behind a
   `&mut Connection`). This is the only place where smoltcp types are referenced.
4. **`DataPlaneConfig::Mode`** — discriminates between smoltcp (default) and
   kernel TUN. When TUN, apps use `std::net::TcpStream` and no `OverlayReadWrite`
   is needed — the kernel routes `10.x.y.z` packets through the TUN fd.
5. **Example** — `foundation_connectrpc/examples/wireguard_echo/` showing:
   - Two `WgNode`s form a mesh (F04 discovery)
   - ConnectRPC server binds an `HttpServer` to the overlay IP
   - ConnectRPC client connects via `H1Transport` → `Connection::Overlay` → `WgHandle::tcp_connect`
   - Unary RPC round-trips over the encrypted tunnel
6. **Tests** — unary echo over overlay `Connection`; WouldBlock retry; connect timeout.

## HOW

### Crate dependency graph (no cycles)

```
foundation_netio           (Defines OverlayReadWrite trait, Connection::Overlay variant)
       ↑
       │ (depends on)
       │
foundation_wireguard       (Implements OverlayReadWrite for overlay streams)
                           (Calls WgHandle::tcp_connect → wraps in Box<dyn OverlayReadWrite>)
       ↑
       │ (depends on — consumer creates WgNode, passes overlay connections to HTTP stack)
       │
foundation_connectrpc      (Uses Connection via H1Transport — zero code changes)
```

The trait lives in `foundation_netio` where `Connection` already lives — the same
pattern as `CompletionReadWrite` in `foundation_iogate`. The impl lives in
`foundation_wireguard` because that's where `OverlayStream` is available.

### `OverlayReadWrite` trait (in `foundation_netio::native::connection`)

```rust
/// WireGuard overlay byte stream (smoltcp userspace TCP/IP, F11).
///
/// Same pattern as [`CompletionReadWrite`]: a boxed trait object so
/// `foundation_netio` does not need to depend on `foundation_nativeapis`.
/// The impl lives in `foundation_wireguard` where `OverlayStream` is available.
pub trait OverlayReadWrite:
    Read + Write + Send + Sync + std::fmt::Debug + 'static
{
    /// Duplicate the underlying stream handle (cheap — `Arc` clone).
    fn clone_box(&self) -> Box<dyn OverlayReadWrite>;
}
```

### `Connection::Overlay` variant (in `foundation_netio`)

```rust
pub enum Connection {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(unix_net::UnixStream),
    Tls(TlsStream),
    #[cfg(unix)]
    Completion(Box<dyn CompletionReadWrite>),
    /// WireGuard overlay TCP stream — smoltcp userspace netstack (F11).
    /// The trait object avoids a `netio → nativeapis → db → netio` cycle.
    Overlay(Box<dyn OverlayReadWrite>),
}
```

### `Read`/`Write`/`AsRawFd` delegation

All existing methods on `Connection` that match on the enum get a new arm:

```rust
impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(t)           => t.read(buf),
            Self::Unix(u)          => u.read(buf),
            Self::Tls(t)           => t.read(buf),
            Self::Completion(c)    => c.read(buf),
            Self::Overlay(s)       => s.read(buf),   // <-- new
        }
    }
}
```

Same for `Write`, `AsRawFd` (return `-1`), `ReadTimeoutOperations` (WouldBlock retry loop),
`PeekableReadStream` (return `None`), `SplitReadStream` (passthrough).

### `OverlayReadWrite` impl (in `foundation_wireguard`)

```rust
use foundation_netio::native::connection::OverlayReadWrite;

/// Wraps a smoltcp `OverlayStream` as an `OverlayReadWrite` trait object.
struct OverlayConnection {
    stream: foundation_nativeapis::dataplane::netstack::OverlayStream,
}

impl Read for OverlayConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)  // OverlayStream::read(&self, buf)
    }
}

impl Write for OverlayConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf) // OverlayStream::write(&self, buf)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl OverlayReadWrite for OverlayConnection {
    fn clone_box(&self) -> Box<dyn OverlayReadWrite> {
        Box::new(OverlayConnection { stream: self.stream.clone() })
    }
}
```

### Connect to an overlay peer (in `WgHandle`)

```rust
impl WgHandle {
    /// Open an overlay `Connection` to a peer — compatible with
    /// the entire netio/ConnectRPC/HTTP stack.
    pub fn overlay_connect(&self, peer_ip: IpAddr, port: u16)
        -> io::Result<Connection>
    {
        let stream = self.netstack.clone().tcp_connect(
            SocketAddr::new(peer_ip, port)
        )?;
        Ok(Connection::Overlay(Box::new(OverlayConnection { stream })))
    }
}
```

### Kernel TUN path

When `DataPlaneConfig::mode = Tun`, `WgNode::join()` opens a `TunDevice` instead of
a `NetStack`. Apps use `std::net::TcpStream::connect("10.x.y.z:8080")` — the kernel
routes `10.0.0.0/8` through the TUN fd. No `OverlayReadWrite` needed.

The `DataPlaneConfig` discriminant:

```rust
pub enum DataPlaneMode {
    /// smoltcp userspace netstack (default, no privileges, cross-platform).
    Smoltcp,
    /// Kernel TUN device (Linux/Darwin, requires CAP_NET_ADMIN or TUN ownership).
    #[cfg(not(target_family = "wasm"))]
    Tun { name: String, mtu: u32 },
}
```

## Task list

1. **Define trait**: `OverlayReadWrite` in `foundation_netio::native::connection`.
2. **Add variant**: `Connection::Overlay(Box<dyn OverlayReadWrite>)`.
3. **Extend Connection methods**: `Read`, `Write`, `AsRawFd` (`-1`), `ReadTimeoutOperations`, `PeekableReadStream`, `SplitReadStream`, `is_unix`.
4. **Impl in wireguard**: `OverlayConnection` wrapping `OverlayStream`, implementing `OverlayReadWrite`.
5. **Add `WgHandle::overlay_connect()`**: opens overlay TCP, returns `Connection::Overlay(...)`.
6. **Add `DataPlaneConfig::Mode`**: smoltcp vs TUN discriminant.
7. **Branch `WgNode::join()`**: smoltcp path creates `NetStack`; TUN path creates `TunDataPlane` and sets up kernel route.
8. **Example**: `foundation_connectrpc/examples/wireguard_echo/` — two-node mesh + unary RPC over overlay.
9. **Tests**: unary echo over `Connection::Overlay`; WouldBlock retry; connect timeout.

## Test plan

- **Unit**: `OverlayConnection` read/write round-trips through an `OverlayStream` bridged by an in-memory packet queue.
- **Integration**: ConnectRPC unary echo client ↔ server over a live two-node WireGuard mesh. Client calls `Echo("hello")`, server echoes back. Verifies end-to-end: WG handshake → smoltcp TCP → HTTP/1.1 → ConnectRPC header/framing → response.
- **TUN mode**: `TunDevice::open` on Linux, read/write raw IP round-trip, overlay socket methods return `Unsupported`.

## Related decisions

- [02 — Dual data plane (smoltcp + TUN)](../../decisions/02-dual-dataplane.md)
- [14 — Crate layering](../../decisions/14-crate-layering.md)
- Feature [04 — Mesh & data-plane orchestration](../04-mesh-and-dataplane-orchestration/feature.md)
- `foundation_iogate` — `CompletionReadWrite` trait-object pattern (the reference for this design)
