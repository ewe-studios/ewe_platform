# Feature 00 — `foundation_nativeapis` Data-Plane Primitives

**Depends on:** none (foundation)
**Unblocks:** 01, 04
**Decisions:** [02](../../decisions/02-dual-dataplane.md), [14](../../decisions/14-crate-layering.md)

## WHY

`boringtun::Tunn` produces/consumes **raw IP packets** on its inner boundary. To turn that into
usable sockets we need a TCP/IP stack. We add **two** into `foundation_nativeapis` — a userspace
netstack (smoltcp) and a kernel TUN device — behind one sans-I/O `DataPlane` trait, so
`foundation_wireguard` (feature 01/04) can select per platform. See
[decision 02](../../decisions/02-dual-dataplane.md) for the full layering rationale.

## WHAT

New in `foundation_nativeapis`:

1. `netstack` module — a smoltcp-backed userspace TCP/IP stack (cross-target, `no_std`-friendly).
2. `tun` module — kernel TUN device wrapper (`cfg(not(wasm32))`, Linux + Darwin).
3. `dataplane` module — the sans-I/O `DataPlane` trait both implement, plus overlay socket handles.

## HOW

### The `DataPlane` trait (sans-I/O)

```rust
/// Sans-I/O L3 data plane: converts overlay sockets <-> raw IP packets.
/// Driven by a valtron task; performs no I/O itself.
pub trait DataPlane {
    /// Feed a decrypted inbound IP packet (from Tunn::decapsulate).
    fn inject_inbound_ip(&mut self, packet: &[u8]);
    /// Advance internal state (timers, TCP state machines). Returns next wake deadline.
    fn poll(&mut self, now: Instant) -> Option<Instant>;
    /// Drain outbound IP packets (to feed Tunn::encapsulate).
    fn drain_outbound_ip(&mut self, f: &mut dyn FnMut(&[u8]));
    /// Overlay socket API.
    fn tcp_listen(&mut self, addr: SocketAddr) -> io::Result<OverlayListener>;
    fn tcp_connect(&mut self, addr: SocketAddr) -> io::Result<OverlayStream>;
    fn udp_bind(&mut self, addr: SocketAddr) -> io::Result<OverlayUdp>;
}
```

### smoltcp `NetStack`

- A smoltcp `phy::Device` whose TX buffer feeds `drain_outbound_ip` and whose RX queue is fed by
  `inject_inbound_ip` — i.e. the "wire" is the `Tunn` plaintext side, **not** a NIC.
- `Interface` at **medium-ip** (L3 overlay; no Ethernet), `SocketSet`, IPv4+IPv6, `socket-tcp` +
  `socket-udp`.
- `poll()` wraps `iface.poll(...)`; overlay socket handles map to smoltcp socket handles; `send/recv`
  bridge `send_slice`/`recv_slice` and register valtron wakers on `can_send`/`can_recv` transitions.
- Feature flags: no smoltcp `std`/`phy-*` (the phy is us); enable `medium-ip`, `proto-ipv4/6`,
  `socket-tcp/udp`, `alloc` where available.

### Kernel `TunDevice`

- `TunDevice::open(TunConfig { name, mtu, ipv6 })`:
  - **Linux:** open `/dev/net/tun`, `TUNSETIFF` with `IFF_TUN | IFF_NO_PI`.
  - **Darwin:** `utun` control socket (`SYSPROTO_CONTROL`, `com.apple.net.utun_control`).
- `read`/`write` raw IP; register the fd with the existing poll reactor (io_uring/epoll) exactly like
  `UdpSocket` (`register`, `AsRawFd`). Implements `DataPlane` by wiring fd reads → `inject_inbound_ip`
  never applies (kernel *is* the stack) — instead the TUN `DataPlane` maps `drain_outbound_ip` to
  writes and inbound decapsulated packets to fd writes; the overlay socket API returns an error
  ("use OS sockets") since in TUN mode apps use the kernel stack directly.
- Reference (do not depend on): `boringtun/src/device/tun_linux.rs` / `tun_darwin.rs`.
- `cfg(not(wasm32))`; wasm build compiles a stub that errors if selected.

## Task list

1. `dataplane` module: `DataPlane` trait + `OverlayStream`/`OverlayListener`/`OverlayUdp` handle
   types (valtron-wakeable).
2. `netstack` module: smoltcp `Device` bridge, `NetStack: DataPlane`, waker registration.
3. `tun` module: Linux + Darwin `TunDevice`, poll-reactor registration, `TunDataPlane: DataPlane`.
4. Cargo features: `netstack` (default), `tun` (native), wasm stubs for `tun`.
5. Tests (`tests/`, `--profile uat`): loopback TCP over `NetStack` (two stacks bridged by an in-memory
   packet queue standing in for Tunn); TUN open/read/write on Linux (gated, needs privileges — mark
   `#[ignore]` in CI without CAP_NET_ADMIN).

## Test plan / success

- `NetStack` A `tcp_connect` → `NetStack` B `tcp_listen`, bytes round-trip, purely in userspace with
  no kernel socket for the overlay range.
- `poll()` returns correct next-wake deadlines; wakers fire on data availability.
- TUN device opens and echoes an IP packet (privileged, gated).
