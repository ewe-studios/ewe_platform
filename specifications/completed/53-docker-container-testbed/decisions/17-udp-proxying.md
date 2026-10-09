# 17 — UDP Proxying

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Add `udp://` backend support — a `UdpPassthrough` primitive that proxies UDP
datagrams byte-for-byte with no HTTP semantics, using the same `BackendProtocol`
inference pattern as the existing `tcp://` support. UDP health probes send a
datagram and wait for a response.

## Why UDP

`tcp://` covers RDP, VNC, and noVNC. The natural sibling is UDP:

| Protocol | Use case |
|----------|----------|
| DNS (port 53) | Internal DNS resolution through the proxy |
| QUIC (HTTP/3) | Future protocol support behind a proxy |
| Game servers | Minecraft Bedrock, Source engine, etc. |
| Syslog (port 514) | Centralized logging |
| STUN/TURN | WebRTC infrastructure |
| TFTP | Network boot, firmware updates |

kamal-proxy does not support any non-HTTP protocol. `foundation_proxy` already
supports `tcp://` (decision 14, "Protocol inference") — UDP is the next step in
the "passthrough" layer.

## Architecture

```
UDP Client (e.g. dig @proxy)
        │
        ▼
  UdpPassthrough (UdpSocket bound to listen_addr)
        │
        │  recv_from(client) → datagram
        │  send_to(backend)  → datagram
        │  recv_from(backend) → response (with timeout)
        │  send_to(client) → response
        │
        ▼
  Backend (udp://host:port)
```

### Differences from TCP passthrough

| Aspect | TCP passthrough | UDP passthrough |
|--------|----------------|-----------------|
| Socket type | `TcpListener` + per-client `TcpStream` | Single `UdpSocket` for all clients |
| Connection model | Connected — one thread per client | Connectionless — one relay per passthrough |
| Data boundaries | Byte stream (no guaranteed boundaries) | Datagram boundaries preserved |
| Readiness | `accept()` + `splice_bidirectional()` | `recv_from()` / `send_to()` loop |
| Shutdown | Close sockets, join threads | Signal probe, join relay thread |

## Design

### Protocol inference (same pattern as tcp://)

```rust
// config.rs
pub enum BackendProtocol {
    Http,
    Https,
    Tcp,
    Udp,   // NEW
}

impl BackendTarget {
    pub fn protocol(&self) -> BackendProtocol {
        let lower = self.url.to_ascii_lowercase();
        if lower.starts_with("udp://") {      // NEW — checked before tcp://
            BackendProtocol::Udp
        } else if lower.starts_with("tcp://") {
            BackendProtocol::Tcp
        } else if lower.starts_with("https://") {
            BackendProtocol::Https
        } else {
            BackendProtocol::Http
        }
    }

    pub fn authority(&self) -> String {
        // Already handles any scheme via split_once("://")
    }
}
```

`authority()` strips the scheme via `split_once("://")` — no change needed.

### UdpPassthrough

```rust
pub struct UdpPassthrough {
    local_addr: SocketAddr,
    shutdown: Arc<OnSignal>,
    relay_thread: Option<JoinHandle<()>>,
}
```

- `start(listen_addr, backend_authority)` — binds `UdpSocket`, spawns relay
- `shutdown()` — turns on signal, joins relay thread
- `local_addr()` — bound address (useful when binding port 0)

**Relay loop** runs on a dedicated thread:
```rust
fn relay_udp(socket: UdpSocket, backend_addr: SocketAddr, shutdown: &OnSignal) {
    let mut buf = [0u8; 65535]; // max UDP datagram
    loop {
        // Client → backend
        match socket.recv_from(&mut buf) {
            Ok((n, client_addr)) => {
                socket.send_to(&buf[..n], backend_addr).ok();
                // Backend → client (with short timeout for shutdown)
                socket.set_read_timeout(Some(Duration::from_millis(100))).ok();
                match socket.recv_from(&mut buf) {
                    Ok((n, _from)) => { socket.send_to(&buf[..n], client_addr).ok(); }
                    Err(ref e) if e.kind() == WouldBlock => {} // timeout, no response yet
                    Err(_) => {}
                }
                socket.set_read_timeout(None).ok(); // restore blocking for next recv
            }
            Err(ref e) if e.kind() == WouldBlock => {}
            Err(_) => break,
        }
        if shutdown.probe() { break; }
    }
}
```

The relay reads a datagram from the client, forwards it to the backend, then
reads a response from the backend (with a short timeout so shutdown is responsive
even when the backend is silent), and sends it back to the originating client.

### Health probes

```rust
fn probe_udp(authority: &str, timeout: Duration) -> bool {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return false,
    };
    socket.set_read_timeout(Some(timeout)).ok();
    // Send an empty datagram — the backend should echo it back.
    if socket.send_to(&[], authority).is_ok() {
        let mut buf = [0u8; 1];
        socket.recv_from(&mut buf).is_ok()
    } else {
        false
    }
}
```

### Handler completeness

`BackendProtocol::Udp` in the HTTP handler's `relay()` function is unreachable in
practice — UDP traffic never enters the HTTP front end. The match arm exists for
exhaustiveness and logs an error if somehow reached, then closes the connection.

## Test strategy

**Unit tests:**
- `BackendTarget::protocol()` recognizes `udp://host:port` → `Udp`
- `BackendTarget::authority()` strips `udp://` correctly

**Docker integration test:**
- Container: `alpine/socat` running `socat UDP-LISTEN:9000,fork EXEC:cat`
- Wait strategy: `WaitFor::Udp` — sends a datagram, expects a response
- Test: binds `UdpPassthrough`, sends a datagram, reads response, asserts match

## Testbed bugs fixed

The Docker testbed already had `PortProtocol::Udp` but four places hardcoded TCP.
These are fixed as part of this change:

1. `config.rs` — `port()`/`port_mapped()` always set `PortProtocol::Tcp` →
   add `port_udp()`/`port_udp_mapped()`
2. `container.rs` — `resolve_ports()` hardcoded `"{port}/tcp"` → use `pm.protocol`
3. `container.rs` — `host_port()` hardcoded `"{port}/tcp"` → add `host_port_udp()`
4. `wait_for.rs` — `WaitFor::Port` uses `TcpStream::connect` → add `WaitFor::Udp`
