# 20 — Unix-socket RPC (Stage 3)

**Date:** 2026-07-10
**Status:** Resolved

## Decision

`foundation_proxy` exposes a Unix-domain socket that accepts JSON-RPC commands
for runtime control: deploy, remove, pause, resume, drain, and status. This is
the control plane that `foundation_deployment_platform` drives during zero-
downtime deploys.

kamal-proxy does the same via `kamal-proxy deploy <service> --target <host:port>`.
The difference: we expose a socket, not a CLI subcommand. The CLI is a thin
client that speaks JSON-RPC over the socket.

## Why

The proxy needs to change its routing table at runtime without restarting:
- A new container starts → register it as a backend
- A health check fails → the proxy already handles this internally (no RPC needed)
- A deploy completes → drain old backends, register new ones
- An operator needs to pause a misbehaving backend

These operations can't be done via static config. The Unix socket is the channel.

## Protocol

JSON-RPC 2.0 over a Unix-domain socket at a well-known path:

```
/var/run/foundation-proxy.sock   (Linux)
~/.foundation/proxy.sock         (macOS / dev)
```

### Commands

```json
// Register a new backend for a service
→ {"jsonrpc": "2.0", "method": "deploy", "params": {
     "service": "app",
     "target": {"url": "http://localhost:3001", "weight": 0},
     "drain_timeout_secs": 30
   }, "id": 1}
← {"jsonrpc": "2.0", "result": {"backend_id": "abc123", "status": "registered"}, "id": 1}

// Remove a backend
→ {"jsonrpc": "2.0", "method": "remove", "params": {
     "service": "app", "backend_id": "abc123"
   }, "id": 2}
← {"jsonrpc": "2.0", "result": {"drained": true, "in_flight": 0}, "id": 2}

// Pause a backend (stop traffic, keep in-flight)
→ {"jsonrpc": "2.0", "method": "pause", "params": {
     "service": "app", "backend_id": "abc123"
   }, "id": 3}
← {"jsonrpc": "2.0", "result": {"paused": true}, "id": 3}

// Resume a paused backend
→ {"jsonrpc": "2.0", "method": "resume", "params": {
     "service": "app", "backend_id": "abc123"
   }, "id": 4}
← {"jsonrpc": "2.0", "result": {"resumed": true}, "id": 4}

// Replace all backends atomically (zero-downtime swap)
→ {"jsonrpc": "2.0", "method": "replace_backends", "params": {
     "service": "app",
     "new_backends": [{"url": "http://localhost:3001"}],
     "drain_timeout_secs": 30
   }, "id": 5}
← {"jsonrpc": "2.0", "result": {"replaced": 1, "drained": 1}, "id": 5}

// Get proxy status
→ {"jsonrpc": "2.0", "method": "status", "params": {}, "id": 6}
← {"jsonrpc": "2.0", "result": {
     "services": {
       "app": {"backends": [
         {"url": "http://localhost:3000", "state": "active", "healthy": true, "inflight": 3}
       ]}
     }
   }, "id": 6}
```

## Implementation

```rust
// foundation_proxy::rpc (new module)

pub struct RpcServer {
    socket_path: PathBuf,
    shutdown: Arc<OnSignal>,
    accept_thread: Option<JoinHandle<()>>,
}

impl RpcServer {
    pub fn start(state: Arc<ProxyState>) -> io::Result<Self>;
    pub fn shutdown(self);
}

// Internal: accept loop on UnixListener, one thread per connection,
// read line-delimited JSON, dispatch to handler, write response.
```

The RPC server holds an `Arc<ProxyState>` — it can directly call
`service.replace_backends()`, `backend.set_state(Paused)`, etc.

## Integration with deployment_platform

```
ContainerGroup::deploy(&proxy_socket, service_name, containers)
  ├── Start containers
  ├── Health-check containers
  ├── RPC: deploy (weight=0) for each new container
  ├── RPC: set weight gradually (canary rollout)
  ├── RPC: replace_backends (atomic swap, drains old)
  └── Stop old containers
```

## Dependencies

| Crate | Role |
|-------|------|
| `foundation_nativeapis` | Unix-domain socket (`std::os::unix::net::UnixListener`) |
| `serde_json` | JSON-RPC serialization |
| `foundation_core::synca::OnSignal` | Shutdown signalling |

## Verification

1. Start proxy with `RpcServer::start()`, send deploy/remove/pause/status
   commands via `nc -U`, verify responses.
2. `replace_backends` atomically swaps: new requests go to new backends,
   old backends drain, in-flight count reaches zero.
3. Invalid method → proper JSON-RPC error response.
