---
feature: "WS Depends read model — caller-injected EventReadiness (D13 E2)"
description: "Caller wraps fd in Arc<dyn EventReadiness>, passes it to the task. Reactor-backed or timer-fallback — task code is identical."
status: "in-progress"
priority: "medium"
phase: 4
depends_on: ["36-ws-resumable-decoder", "10-reactor-parking", "02-pipe-primitive"]
estimated_effort: "medium"
created: 2026-07-03
updated: 2026-07-10
---
# Feature 38: WS Depends read model — caller-injected EventReadiness

## Description

True parking for WebSocket reads on native — the caller wraps the fd in an
`Arc<dyn EventReadiness>` and passes it to the task. The task returns
`TaskStatus::Depends(readiness)`. What the caller passes determines the
parking strategy: a `RegisteredFd` for kernel-level epoll parking, or a
timer-based fallback. The task never touches the fd directly.

## Design: caller-injected EventReadiness (2026-07-10)

### The idea

Don't make the task extract the fd, register it with a reactor, and manage
the registration lifecycle. Instead, the **caller** does all of that **before**
constructing the task, wraps the result in an `Arc<dyn EventReadiness>`, and
hands it in.

```
Caller (Transport::open, WsTransport, test harness)
│
├── Has a reactor?
│   ├── YES:  RegisteredFd::new(tcp_stream, &registry, token)?
│   │         → Arc<dyn EventReadiness>  (kernel epoll/kqueue, real parking)
│   │
│   └── NO:   TimerReadiness::new(Duration::from_millis(5))
│             → Arc<dyn EventReadiness>  (returns true every 5ms, never blocks)
│
└── Passes into task constructor:
    WebSocketTask::new(stream, config, pipe_rx, Some(readiness))
    WebSocketServerTask::new(stream, config, pipe_tx, Some(readiness))
```

The task holds `Option<Arc<dyn EventReadiness + Send + Sync>>`. It never
creates it, never registers/deregisters it. It just uses it.

### Why this is better

| Concern | Task-internal (bad) | Caller-injected (good) |
|---|---|---|
| **Who owns the fd?** | Task borrows it, must deregister before Drop | Caller owns both the TcpStream and the Registration — drops them together |
| **Lifetime** | Task must track registration state, handle re-registration after EPOLLONESHOT | Caller handles the full lifecycle. Task drops its Arc when done — refcount handles cleanup |
| **Testability** | Can't swap the reactor in tests without global state | Test passes `Arc::new(AlwaysReady)` — no reactor needed |
| **Portability** | Task must know about epoll/kqueue/uring differences | Caller picks the right impl per platform. Task code is identical everywhere |
| **Fallback** | Task checks `Reactor::get()` on every poll | Caller picks the fallback once at construction time. No per-poll branching |

### Concrete types

```rust
// ── Production: reactor-backed parking ──
use foundation_nativeapis::native::fd::{RegisteredFd, Interest, Token};
use foundation_nativeapis::native::poll::Poll;

let poll = Poll::new()?;
let registry = poll.registry();
let fd = RegisteredFd::new(tcp_stream, &registry, Token(0), Interest::READABLE)?;
let readiness: Arc<dyn EventReadiness + Send + Sync> = Arc::new(fd);

let task = WebSocketServerTask::new(stream, config, pipe_tx, Some(readiness));

// ── Fallback: timer-based ──
// Returns true every `interval`, never touches a kernel fd.
// Implements EventReadiness so the task uses Depends, not Delayed.
pub struct TimerReadiness {
    interval: Duration,
    last_ready: Mutex<Instant>,
}

impl EventReadiness for TimerReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        let mut last = self.last_ready.lock().unwrap();
        if last.elapsed() >= self.interval {
            *last = Instant::now();
            true
        } else {
            false
        }
    }
}

let readiness: Arc<dyn EventReadiness + Send + Sync> =
    Arc::new(TimerReadiness::new(Duration::from_millis(5)));

let task = WebSocketServerTask::new(stream, config, pipe_tx, Some(readiness));
```

### How the task uses it

```rust
impl TaskIterator for WebSocketServerTask {
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<...>> {
        // 1. Outbound pipe first — messages are time-sensitive.
        //    (server task: inbound sender; client task: outbound receiver)
        match self.pipe_half.try_op() {
            Ok(data) => return self.process(data),
            Err(Empty) => {}  // fall through
            Err(Closed) => return self.drain(),
        }

        // 2. Try to read a frame.
        match self.decoder.step(&mut self.stream) {
            Ok(Frame(f)) => return self.process(f),
            Ok(Pending) => {}  // no bytes yet — fall through to park
            Err(_) => return self.drain(),
        }

        // 3. Park on the composite of pipe readiness + fd readiness.
        //    If the caller gave us a readiness signal, compose it with
        //    the pipe's QueueReadiness so EITHER signal unparks us.
        let signal: Arc<dyn EventReadiness + Send + Sync> = match &self.fd_readiness {
            Some(fd) => Arc::new(AnyReadiness::new(
                self.pipe_half.readiness(),
                Arc::clone(fd),
            )),
            None => self.pipe_half.readiness(),
        };
        Some(TaskStatus::Depends(signal))
    }
}
```

When `self.fd_readiness` is `Some(registered_fd)`: the task parks until the
kernel signals the fd OR the pipe producer sends. When `None`: parks only on
the pipe. The task body is identical either way.

### Caller lifecycle

```
Transport::open():
  1. TcpStream::connect(addr)
  2. (optional) RegisteredFd::new(&stream, &reactor_registry, token, interest)?
  3. Create Pipe pair
  4. Spawn WebSocketTask { stream, pipe_rx, fd_readiness: Some(registered_fd) }
  5. Return TransportStream { send_body: pipe_tx, ... }

On TransportStream drop:
  - pipe_tx drops → task's pipe_rx.try_recv() → Closed → task drains
  - task drops → registered_fd Arc refcount decrements
  - last Arc drop → RegisteredFd::drop deregisters from epoll
  - TcpStream closes
```

### Integration with F37 Pipe migration

When both pipe parking and fd parking are active, the task composes them:

- **Pipe has data + fd not ready**: `try_recv()` succeeds → process outbound immediately. Never parks.
- **Fd ready + pipe empty**: `decoder.step()` reads bytes → processes inbound frame. Never parks.
- **Both idle**: `TaskStatus::Depends(AnyReadiness::new(pipe.readiness(), fd_readiness))` — parks on the composite. First one to fire unparks the task.

With F40's shared reactor, `fd_readiness.is_ready()` reads a cached atomic
bit — zero syscalls on the check path.

## Scope

- [ ] `TimerReadiness` — simple `EventReadiness` impl that returns `true` every N ms. Used when no reactor is available.
- [ ] `WebSocketServerTask`, `WebSocketTask` accept `Option<Arc<dyn EventReadiness + Send + Sync>>` in constructor
- [ ] `ReconnectingWebSocketTask` forwards the readiness signal to each new inner task
- [ ] Task `next_status()`: check pipe → check frame → park on composite of pipe.readiness() + fd_readiness
- [ ] Caller (Transport, test harness) owns the registration lifecycle
- [ ] `Spawner = BoxedSendExecutionAction` on all tasks (never `NoSpawner`)

## Acceptance criteria

- Idle WS connection holds no worker when `RegisteredFd` is provided — parks via `Depends(registered_fd)`
- `TimerReadiness` fallback: parks via `Depends(timer)`, wakes every N ms — same task code, different signal
- Pipe wake + fd wake compose correctly via `AnyReadiness` — a message arriving during a parked fd read unparks the task immediately
- Test passes with `Arc::new(AlwaysReady)` — no kernel fd, no epoll, pure unit test
