# Kernel TUN Device — Setup Guide

`foundation_wireguard` supports two data planes (decision 02):

| Mode | How apps connect | Privileges | Platform |
|------|-----------------|------------|----------|
| **smoltcp** (default) | `WgHandle::tcp_connect()` → `OverlayStream` | None | All (native + wasm) |
| **kernel TUN** (opt-in) | `std::net::TcpStream::connect("10.x.y.z:port")` | `CAP_NET_ADMIN` or TUN device ownership | Linux/Darwin only |

The kernel TUN mode routes overlay IP packets through the kernel TCP/IP stack, so
any application that can open a TCP socket works over the WireGuard mesh — no
`OverlayStream` needed. ConnectRPC, gRPC, `curl`, `wget`, `nc` — all work
natively.

## One-time system setup

### Linux

```bash
# 1. Load the TUN kernel module (survives reboots — usually already loaded)
sudo modprobe tun

# 2. Create a persistent TUN device owned by your user
sudo ip tuntap add dev wg-ewe0 mode tun user $(whoami)

# 3. Bring the interface up (no IP yet — WireGuard will configure it)
sudo ip link set wg-ewe0 up
```

Verify:

```bash
ip link show wg-ewe0
# Should show: wg-ewe0: <NO-CARRIER,UP,...> state DOWN
# DOWN is fine — WireGuard brings it UP when the mesh starts.
```

### Darwin (macOS)

The TUN driver uses `com.apple.net.utun_control` — no setup needed. The kernel
assigns the next available `utunN` name automatically.

## Using the TUN data plane

Set `dataplane = "tun"` in `wireguard.toml`:

```toml
[dataplane]
mode = "tun"
name = "wg-ewe0"
mtu = 1420
```

Or via the builder:

```rust
let config = WgConfig::builder()
    .seed(my_seed)
    .dataplane_tun("wg-ewe0", 1420)
    .build()?;
```

After `WgNode::join()`, the kernel routes `10.x.y.z` traffic through the TUN
device. Apps use standard OS sockets:

```rust
// This works when the data plane is kernel TUN:
let stream = std::net::TcpStream::connect("10.0.0.2:8080")?;
```

When using smoltcp (default, no privileges), use `WgHandle::tcp_connect()` instead.

## Troubleshooting

**`EPERM` when opening `/dev/net/tun`:**
Either run as root or create the device with `user $(whoami)` as shown above.

**Interface is DOWN after join:**
WireGuard only brings the TUN device UP when a peer is online. If no peers are
connected, the interface stays DOWN.

**No route to 10.x.y.z:**
On Linux, `WgConfig` adds the overlay route when the TUN is opened. If your
routing table doesn't show the route:

```bash
ip route add 10.0.0.0/8 dev wg-ewe0
```

The code does this automatically on Linux — no manual route needed in normal
operation.
