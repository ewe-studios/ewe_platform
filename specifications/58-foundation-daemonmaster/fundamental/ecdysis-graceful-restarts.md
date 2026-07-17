# Exploration: ecdysis — Graceful Restarts via Socket Inheritance

**Source:** Cloudflare, Inc. ([github.com/cloudflare/ecdysis](https://github.com/cloudflare/ecdysis))
**Crate:** `ecdysis` v1.1.1 · Apache-2.0 · Linux-focused (tested on Debian Bullseye/Bookworm/Trixie)
**Inspired by:** Cloudflare's Go library [tableflip](https://github.com/cloudflare/tableflip)

Sources:  /home/darkvoid/Boxxed/@formulas/src.rust/src.mise/
---

## Summary

Ecdysis solves the problem of **updating running network services without disrupting active connections**. The core mechanism is a fork/exec cycle where the parent passes all listening socket file descriptors to the child through a pipe, the child reconstructs listeners from those FDs, declares readiness, and only then does the parent stop accepting new connections and drain existing ones.

The name comes from biology — *ecdysis* means the process of shedding an old skin, which is exactly what this library enables a process to do.

---

## Core Architecture

### The Two-Process Handshake

The upgrade lifecycle follows this sequence:

```
Parent (running)                          Child (spawned)
─────────────────                         ─────────────────
1. Ecdysis::upgrade()
   - Creates two pipes:
     a) listener_fds pipe (parent→child)
     b) ready_notify pipe (child→parent)
   - Spawns child via fork/exec
   - Sets env vars:
     ECDYSIS_UPGRADE=1
     ECDYSIS_PIPE_FDS=<fd>
     ECDYSIS_PIPE_READY=<fd>
   ──────────────────────────────────────────▶
                                             2. init_child()
                                                - Reads ECDYSIS_UPGRADE env
                                                - Deserializes ListenerInfo[]
                                                  from pipe
                                                - Returns (Vec<ListenerInfo>,
                                                  ready_pipe)
   ◀──────────────────────────────────────────
                                             3. Child calls ecdysis.ready()
                                                - Writes "OK" to ready pipe
   ──────────────────────────────────────────▶
4. Parent receives "OK"
   - Stops accepting new connections
   - Waits for existing connections to drain
   - Exits cleanly
```

### Key Files

| File | Role |
|------|------|
| `src/lib.rs` | `Ecdysis` struct — entry point, listener creation API |
| `src/executioner.rs` | Fork/exec mechanics, pipe setup, child monitoring |
| `src/inheriter.rs` | Child-side environment parsing, FD deserialization |
| `src/registry.rs` | `ListenerRegistry` — dual-vec FD tracking |
| `src/listener.rs` | `Listener` trait for socket type abstraction |
| `src/tokio_ecdysis/mod.rs` | Async Tokio wrapper, supervisor, trigger system |
| `src/tokio_ecdysis/supervisor.rs` | `StoppableStream` wrapper with tokio::select! cancellation |
| `src/tokio_ecdysis/trigger.rs` | Signal/socket triggers for upgrade initiation |
| `src/tokio_ecdysis/systemd_notify.rs` | systemd READY=1 / RELOAD=1 / STOPPING=1 |
| `src/tokio_ecdysis/systemd_sockets.rs` | LISTEN_FDS / LISTEN_PID / LISTEN_FDNAMES parsing |
| `src/seqpacket.rs` | Linux-specific SOCK_SEQPACKET support |

---

## Detailed Mechanisms

### 1. File Descriptor Registry (`registry.rs`)

The registry maintains **two separate vectors** of `ListenerInfo` (fd + SockInfo):

- **`used_fds`** — sockets created by `Ecdysis::listen_*` calls. These get serialized and sent to the child during upgrade.
- **`inherited_fds`** — sockets received from the parent during an upgrade. Used by `inherit()` to reclaim the same socket.

The key design: when a socket is created via `Ecdysis::listen_tcp(addr)`, the registry first checks `inherited_fds` for a matching `SockInfo::Tcp(addr)`. If found, it returns that FD (the child reuses the parent's socket). If not found, it creates a new socket and adds a **dup'd copy** of the FD to `used_fds` (the dup preserves FD attributes and ensures the child gets its own reference).

```rust
pub(crate) struct ListenerRegistry {
    inherited_fds: Mutex<Vec<ListenerInfo>>,  // from parent during upgrade
    used_fds:      Mutex<Vec<ListenerInfo>>,  // created by this process
}
```

**SockInfo variants** (serializable via bincode):
- `Unix(Option<PathBuf>)` — Unix domain stream socket
- `Tcp(SocketAddr)` — TCP listener
- `Udp(SocketAddr)` — UDP socket
- `UnboundUnixDatagram(String)` — pathless datagram pair for inter-process comms
- `UnixSeqpacket(Option<PathBuf>)` — Linux SOCK_SEQPACKET

### 2. The Upgrade Execution (`executioner.rs`)

The `upgrade()` function orchestrates the entire fork/exec:

**Pipe topology (4 FDs):**
```
listener_pipes:  (recv_listeners_fd, send_listeners)
                  ← child reads ListenerInfo[] from here
ready_pipes:     (recv_ready, send_ready_fd)
                  ← parent reads "OK" from here
```

**Key design decisions:**

- **`UPGRADING` AtomicBool** — prevents concurrent upgrades (`Already in upgrade` error) and doubles as a cancellation signal for the `wait_child` thread.
- **Thread-per-pipe pattern** — three threads: `send_fds` (serializes ListenerInfo[] via bincode), `wait_child` (polls child exit with 5s timeout), `wait_ready` (reads "OK" from pipe). The `waitr` (ready) thread is the arbiter — when it resolves, the other threads are cancelled.
- **`pre_exec` hook** — after fork but before exec, the CLOEXEC bit is cleared on inherited FDs so they survive into the new process image. Since CLOEXEC is per-FD (not per-socket), and this runs in the child process, there's no FD leak risk.
- **Child timeout** — if the child doesn't send "OK" within 5 seconds, the parent kills it and continues running. This means **crashing during initialization is safe**.
- **PID file atomism** — PID is written to a temp file, then atomically moved to the target path, preventing stale PID scenarios.

### 3. Child Initialization (`inheriter.rs`)

The child detects it's an upgrade via the `ECDYSIS_UPGRADE=1` environment variable:

```rust
pub fn init_child() -> Result<(Vec<ListenerInfo>, os_pipe::PipeWriter), InheritError>
```

1. Check `ECDYSIS_UPGRADE` env var → `NotAnUpgrade` if absent
2. Read FD number from `ECDYSIS_PIPE_FDS` → deserializes `Vec<ListenerInfo>` via bincode
3. Read FD number from `ECDYSIS_PIPE_READY` → returns the pipe writer for `ready()`

This is called automatically by `Ecdysis::new()` — the constructor does the detection internally and sets up the right state.

### 4. Tokio Integration (`tokio_ecdysis/`)

The async layer wraps everything in Tokio patterns:

**`TokioEcdysisBuilder`** — fluent builder that configures triggers before readiness:

```rust
let (ecdysis, upgrade_fut) = TokioEcdysisBuilder::new(SignalKind::Hangup)?
    .stop_on_signal(SignalKind::Interrupt)?     // SIGINT → clean shutdown
    .upgrade_on_socket("/var/run/app/upgrade")?  // UDS connection → upgrade
    .partial_stop_on_signal(SignalKind::Usr1)?   // SIGUSR1 → partial stop
    .enable_systemd_notifications()?
    .ready()?;
```

**`StoppableStream<S>`** — wraps any `Stream` (TCP, Unix, UDP) with supervisor awareness:

```rust
fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
    // Check supervisor state via watch channel
    while let Poll::Ready(Some(state)) = self.rx.poll_next_unpin(cx) {
        self.state = state;
    }
    // If stopping, return None (stream ends) → callers see EOF
    if self.stop_on_shutdown.should_continue(&self.state) {
        self.stream.poll_next_unpin(cx)
    } else {
        Poll::Ready(None)
    }
}
```

**`Stoppable<S>`** — wraps individual sockets with `until_stopped()` method that uses `tokio::select!` to race the operation against a stop signal.

**`Supervisor`** — uses `tokio::sync::watch` channel to broadcast stop state to all supervised streams simultaneously. `stop_all(ExitCondition)` sends the signal; all streams see it on their next poll.

### 5. systemd Integration

Two orthogonal features:

- **`systemd_notify`** — sends `READY=1`, `RELOADING=1`, `STOPPING=1` via `sd_notify()`. Requires `Type=notify-reload` in the unit file. The notifier verifies connectivity *before* the child declares readiness — avoiding situations where the parent thinks the child is ready but the child crashes because it can't talk to systemd.

- **`systemd_sockets`** — reads `LISTEN_FDS`, `LISTEN_FDSNAMES`, `LISTEN_PID` to inherit sockets that systemd pre-opened. Named sockets are matched by name; duplicates and special names (like `sd-daemon` internals) are ignored. After the first read, sockets are added to the registry so they get inherited across upgrades too.

### 6. Trigger System

Upgrades and shutdowns can be initiated by multiple sources:

| Trigger Type | Mechanism | Configurable Actions |
|---|---|---|
| Unix signal | `signal(SignalKind)` stream | Upgrade, Stop, PartialStop |
| Unix socket | Connection to UDS path | Upgrade, Stop, PartialStop |

The trigger system polls all registered triggers concurrently via `poll_triggers()` — the first one to fire wins. Socket triggers keep the connection open (`mem::forget`) until the full shutdown completes, preventing the triggerer from seeing a premature close.

### 7. Inter-Generation Communication

`unix_datagram_pair()` creates a **chain of datagram pairs** across successive upgrades:

```
Parent's pair end ────→ passed to child as ParentPairEnd
Child creates pair ────→ one end returned to app, other end passed to next child
```

This enables state transfer between generations (e.g., connection counts, in-flight request tracking) without requiring external storage.

---

## Design Goals vs. Reality

| Goal | How Ecdysis Achieves It |
|------|------------------------|
| No old code runs after upgrade | Parent exits after child declares ready |
| Child gets grace period for init | Child calls `ready()` only after setup; parent waits up to 5s |
| Crash during init is OK | Parent keeps serving; kills timed-out child, continues |
| Single upgrade at a time | `UPGRADING` AtomicBool gate |
| No connection drops | Parent drains existing connections after child is ready |

---

## Limitations

- **Linux-only** for full functionality (SOCK_SEQPACKET, systemd)
- **No Windows support** — the fork/exec model doesn't translate
- **Bincode serialization** — not self-describing; new `SockInfo` variants must be appended for forward compatibility
- **Manual data passing** — arbitrary data transfer between parent/child requires the user to serialize through named pipes (vs. tableflip which has built-in state transfer)

---

## Comparison with shellflip

| Aspect | ecdysis | shellflip |
|--------|---------|-----------|
| Focus | Socket inheritance & rebinding | Arbitrary data transfer |
| systemd | Optional (feature-gated) | Assumed (opinionated) |
| Runtime | Sync or Tokio (feature) | Tokio required |
| Data passing | User serializes via pipe | Built-in abstractions |
| Socket types | TCP, UDP, Unix, Seqpacket | TCP, Unix |

---

## Key Takeaways for Our Platform

1. **Socket inheritance is the right pattern** for zero-downtime restarts — FD passing via fork/exec + `pre_exec` CLOEXEC manipulation is battle-tested at Cloudflare scale.

2. **The registry pattern** (dual-vec: used vs. inherited) cleanly handles the "am I the first process or a child?" question without external coordination.

3. **Ready-notifier pipe** is simple and effective — the child writes "OK" when prepared, parent kills after timeout. No complex handshakes.

4. **Stoppable streams via watch channel** — the supervisor pattern where all listeners share a single `watch::Sender<RunState>` is elegant and composable. Each stream independently decides when to stop based on the shared state.

5. **Partial stop** concept is interesting — stop some listeners while keeping others running, with systemd notification responsibility delegated to the caller.

6. **The `unix_datagram_pair()`** pattern for inter-generation state transfer is clever but adds complexity. For our use case, we might prefer an external state store or the pipe-based approach.
