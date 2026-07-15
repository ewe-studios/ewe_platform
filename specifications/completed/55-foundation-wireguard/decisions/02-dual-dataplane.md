# 02 — Dual Data Plane: smoltcp Userspace Netstack **and** Kernel TUN

**Date:** 2026-07-12
**Status:** Resolved (owner-confirmed)

## Decision

Ship **both** data planes, and add **both** to `foundation_nativeapis` as reusable primitives:

1. **Userspace netstack** — vendor **smoltcp** into `foundation_nativeapis` to provide an in-process
   TCP/IP stack. **Default** data plane. Portable (native + wasm + Android + iOS), **unprivileged**.
2. **Kernel TUN device** — a `foundation_nativeapis` wrapper over `/dev/net/tun` (Linux) and the
   `utun` control socket (Darwin). **Opt-in.** Native-only, **privileged** (`CAP_NET_ADMIN`/root),
   but **transparent** to any process on the host.

`foundation_wireguard` consumes both behind one sans-I/O `DataPlane` abstraction and selects per
platform / per config: smoltcp by default, kernel TUN when explicitly requested and available.

## Why — the layering that forces this choice

`boringtun::Tunn` is a **pure crypto state machine**. It has two boundaries:

- **Outer (Tunn ⇄ wire):** ciphertext over UDP. **Already fully covered** by
  `foundation_nativeapis::UdpSocket` + `foundation_netio` + io_uring/epoll. smoltcp adds nothing
  here.
- **Inner (app ⇄ Tunn):** `encapsulate()` demands a **fully-formed raw IP packet** and
  `decapsulate()` returns one. **Something must be the TCP/IP stack** that produces/consumes those
  L3 packets. Nothing in our stack does this today.

There are exactly two ways to source/sink those IP packets:

- **Kernel TUN:** the OS kernel *is* the TCP/IP stack. We `read()` raw IP off the TUN fd → `Tunn` →
  UDP, and reverse. Any app uses normal OS sockets bound to the tunnel range. **Needs privileges,
  native-only.**
- **Userspace netstack (smoltcp):** we run the TCP/IP protocol **in-process** on in-memory buffers,
  so services get real sockets **without a kernel interface and without privileges** — the **only**
  option that works on **wasm/mobile** or in an unprivileged container.

**Key clarification (recorded because it recurs):** io_uring and epoll are *readiness/completion*
mechanisms, **not a protocol**; and `foundation_nativeapis`/`foundation_netio` sockets are handles
into the **kernel's** TCP/IP stack. smoltcp is the **protocol implementation itself**, running in
our process. They are complementary, not substitutes. What we are "missing" is not a syscall wrapper
(we have io_uring/epoll/UDP) — it is a **protocol stack**, and reimplementing correct TCP
(retransmit, RTO, windowing, congestion) is a large, bug-prone undertaking, so we use smoltcp
rather than write our own.

Shipping **both** gives the best of both worlds the owner asked for: the portable, unprivileged,
cross-platform default (smoltcp), and the transparent "any app just works" native mode (TUN).

## smoltcp facts (verified against vendored 0.8.0)

- **No async runtime. No tokio. No `futures` in core.** Deps: `managed`, `byteorder`, `bitflags`
  (+ optional `log`, `libc`, `defmt`, `rand_core`). `no_std`, heap-optional.
- **Sans-I/O poll model.** You own the loop: `iface.poll(now, &mut device, &mut sockets)` drains
  inbound packets from the `Device`, advances every socket's TCP state machine, and emits outbound
  packets. Sockets are buffer-based and non-blocking (`can_send`/`send_slice`/`can_recv`/`recv_slice`).
  `poll_at()`/`poll_delay()` tell you when to poll next for timers.
- The `"async" = []` feature is an empty marker (Waker helpers), **not** a runtime.

## What we do

### In `foundation_nativeapis`

- **smoltcp phy Device** whose "wire" is the **`Tunn` plaintext side**: `transmit()` hands an
  outbound IP packet to WG `encapsulate()`; `receive()` yields IP packets from WG `decapsulate()`.
  Expose an `Interface` + `SocketSet` and a `NetStack` type offering `tcp_listen`, `tcp_connect`,
  `udp_bind` that return our stream/datagram handles over the overlay.
- **Kernel TUN** wrapper: `TunDevice::open(name, mtu)` → Linux `/dev/net/tun` (`IFF_TUN|IFF_NO_PI`)
  / Darwin `utun`; `read`/`write` raw IP; registers with the poll reactor (io_uring/epoll) like
  `UdpSocket` does. Target-gated `cfg(not(wasm32))`; stubbed on wasm.
- A **sans-I/O `DataPlane` trait**: `poll(now) -> next_wake`, `inject_inbound_ip(&[u8])`,
  `drain_outbound_ip(&mut FnMut)`, plus the socket-facing API. Both smoltcp and TUN implement it.

### In `foundation_wireguard`

- One **valtron task per tunnel/mesh** drives the loop: wake on UDP readiness (io_uring/epoll) and
  on `min(Tunn::update_timers deadline, DataPlane::poll_at)`; on each wake, decapsulate inbound UDP
  → `DataPlane::inject_inbound_ip`; `DataPlane::poll`; `DataPlane::drain_outbound_ip` → `encapsulate`
  → UDP `send_to`. No runtime conflict (unlike bollard/tokio in spec 53/54).

## Consequences

- Default mode means **services use our socket API** (`NetStack::tcp_connect(...)`) rather than the
  OS's — acceptable and idiomatic for foundation services; the TUN mode covers the "unmodified app"
  case.
- Android/iOS use the **smoltcp** default; their platform VPN TUNs (`VpnService`/`NetworkExtension`)
  are a later optional adapter, not required for connectivity.

## Related decisions

- [01 — Source crates & pinning](01-source-crates-and-pinning.md)
- [14 — Crate layering](14-crate-layering.md)
- Feature [00 — nativeapis data-plane primitives](../features/00-nativeapis-dataplane-primitives/feature.md)
