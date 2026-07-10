---
feature: "WebSocketServerTask + WsServerConfig + Pipe migration"
description: "Progress-driven server task with Pipe seam; ConcurrentQueue→Pipe migration for wake+backpressure"
status: "in-progress"
priority: "medium"
phase: 4
depends_on: ["36-ws-resumable-decoder", "02-pipe-primitive"]
estimated_effort: "medium"
created: 2026-07-03
updated: 2026-07-10
---
# Feature 37: WebSocketServerTask + Pipe migration

## Description

The robust server-side WS task, plus the ConcurrentQueue→Pipe migration that
gives all three WS task families (WebSocketServerTask, WebSocketTask,
ReconnectingWebSocketTask) executor-integrated waking and backpressure.

## Design revision: ConcurrentQueue → Pipe (2026-07-10)

### Current state

All three WS task types use `Arc<ConcurrentQueue<WebSocketMessage>>` for
message delivery:

| Task | Queue role | Problem |
|---|---|---|
| `WebSocketTask` (client) | `delivery_queue` — caller pushes outbound messages, task pops them to write TCP | On `pop() → Empty`: task must return `Delayed` / re-poll. No wake when the caller pushes data — the task spins or sleeps until the next scheduler cycle. |
| `WebSocketServerTask` (server) | `delivery` — task pushes inbound messages, caller drains them | On `push() → Full`: task must buffer or drop. No wake when the consumer drains — the task retries next poll. |
| `ReconnectingWebSocketTask` | Inherits `WebSocketTask`'s queue. Reconnection provides a new inner task with a **new** queue — the old one is abandoned (caller must detect and re-subscribe). | Caller has no signal that reconnection happened; message routing across reconnect boundaries is caller-side boilerplate. |

### Target state: `Pipe<WebSocketMessage>` throughout

`Pipe<T>` (F02) is the bounded two-sided waker-hooked pipe already used by
`FramePipe`, `ByteSink`, `ByteSource`, `send_body`, `recv_body`, `head_rx`,
and `trailers` — every other seam in the transport stack.

| Property | `ConcurrentQueue` | `Pipe<T>` |
|---|---|---|
| **Consumer empty** (task path) | `pop()` returns `None` → task retries next poll (polling or `Delayed`) | `try_recv()` returns `Empty` → task returns `TaskStatus::Depends(pipe.readiness())`. Producer's `try_send()` (or async `send().await`) fires the stashed consumer wake — executor re-polls. Zero wasted polls. |
| **Producer full** (task path) | `push()` returns `Err(Full)` → drop or buffer (no backpressure integration) | `try_send()` returns `Full` → task returns `TaskStatus::Depends(pipe.vacancy())`. Consumer's `try_recv()` (or async `receive().await`) fires the stashed producer wake. Natural backpressure. |
| **Consumer empty** (async path) | N/A — no waker stashes | `receive().await` parks the future on the pipe's consumer waker; producer's `send()` wakes it. |
| **Producer full** (async path) | N/A | `send().await` parks the future on the pipe's producer waker; consumer's `receive()` wakes it. |
| **End-of-stream** | No built-in signal. Caller must set a separate flag or send a sentinel message. | `PipeSender` drop → `PipeReceiver.receive()` returns `None` (async) / `try_recv()` returns `Closed` (task). Clean, automatic. |
| **Cancel composition** | None. | `send()` / `receive()` compose the caller's `CancelSignal` — a cancel wakes the parked future via the shared signal's atomic flip. `try_send()` / `try_recv()` don't cancel (they're non-blocking); the task composes `CancelSignal` with the pipe's `QueueReadiness` via `AnyReadiness` (F02). |
| **Depth** | Configurable (`bounded`/`unbounded`) | Configurable (default 4, F02) |

### How each task type changes

#### Pipe ownership model — caller-supplied or internal

Every task that uses a `Pipe` takes an `Option<PipeHalf>` — if the caller
passed `Some(half)`, the task uses it; if `None`, the task creates a fresh
`Pipe` pair and **exposes the other half** via an accessor. This is the
standard pattern across the crate (see `Transport::open()` — the caller
creates pipes, passes the internal halves to the pump, keeps the external
halves for itself).

```
// Caller creates the Pipe, keeps rx, gives tx to the task:
let (tx, rx) = Pipe::<WebSocketMessage>::with_depth(16);
let task = WebSocketServerTask::new(stream, config, tx);  // task owns tx (inbound sender)
// caller owns rx — drains messages via rx.receive().await

// OR: Task creates the Pipe, caller reads the other half from the task:
let task = WebSocketServerTask::new(stream, config, None);  // task creates pipe internally
let rx = task.inbound_receiver();  // caller gets the read half
```

This is important because `WsTransport::open()` needs to hold `rx` as part of
`TransportStream::recv_body` (bridged through a collector). If the task
created the pipe internally, the caller must be able to reach the other half.

The same applies to `WebSocketTask` (client) for outbound:
```
// Caller creates the Pipe, keeps tx, gives rx to the task:
let (tx, rx) = Pipe::<WebSocketMessage>::with_depth(16);
let task = WebSocketTask::connect_with_pipe_resolver(url, rx)?;
// caller owns tx — sends messages via tx.send(msg).await

// OR: Task creates the Pipe, caller reads the other half:
let task = WebSocketTask::connect(resolver, url)?;
let tx = task.outbound_sender();  // caller gets the write half
```

#### WebSocketServerTask

```
Before:
  task pushes into Arc<ConcurrentQueue<WebSocketMessage>>  (inbound, task→caller)
  caller drains from the same Arc

After:
  Caller supplies Option<PipeSender<WebSocketMessage>>:
    Some(tx)  → task uses it, caller already holds the matching rx
    None      → task creates a Pipe pair, exposes rx via inbound_receiver()

  task.next_status():
    assembles message → tx.try_send(msg)
      Ok → done.
      Full → TaskStatus::Depends(pipe.vacancy())  // parks until consumer drains
    on Close → drop(tx)                           // caller's rx.try_recv() → Closed
```

#### WebSocketTask (client)

This is the existing task in `foundation_netio` — authored before F02 Pipe
existed. Migration:

```
Before:
  task pops from Arc<ConcurrentQueue<WebSocketMessage>>  (outbound, caller→task)
  caller pushes into the same Arc

After:
  Caller supplies Option<PipeReceiver<WebSocketMessage>>:
    Some(rx)  → task uses it, caller already holds the matching tx
    None      → task creates a Pipe pair, exposes tx via outbound_sender()

  task.next_status():
    match rx.try_recv() {
      msg → write WS Binary frame to TCP
      Empty → return TaskStatus::Depends(pipe.readiness())  // parks on QueueReadiness
      Closed → drain + close TCP (caller dropped tx)
    }
```

`try_recv()` is the task-path counterpart to `receive().await` — non-blocking,
returns `Empty` when the pipe has no data, and the task parks via
`TaskStatus::Depends(pipe.readiness())`. When the caller calls `tx.send()` or
`tx.try_send()`, the pipe fires the stashed consumer waker, the executor
re-polls the task, and `try_recv()` returns the message.

#### ReconnectingWebSocketTask

Wraps `WebSocketTask` and inherits its Pipe ends. Key benefit:
**reconnection transparency for the caller**.

```
Before:
  Each reconnect creates a new inner task with a new Arc<ConcurrentQueue>.
  The caller must detect reconnection and re-subscribe — the old queue is
  orphaned, messages sent to it after disconnect are silently lost.

After:
  The caller holds the SAME PipeSender<WebSocketMessage> across reconnects.
  On disconnect: the inner task's PipeReceiver drops, the old task drains.
  On reconnect: a new WebSocketTask is created with a fresh Pipe pair.
  The caller's PipeSender is SWAPPED atomically — it points at the new
  pipe, and messages already queued in the old pipe are drained first.

  ReconnectingWebSocketTask stores:
    outbound_tx: Arc<Mutex<PipeSender<WebSocketMessage>>>  // shared with caller
    outbound_rx: Option<PipeReceiver<WebSocketMessage>>    // current inner task's rx

  On reconnect:
    1. Create new Pipe pair.
    2. Swap the shared tx to point at the new pipe.
    3. Spawn a drainer: empty any remaining messages from the OLD rx,
       forward them to the new tx (preserves in-flight messages).
    4. Give the new rx to the fresh inner WebSocketTask.
    5. Drop the old inner task.

  Caller sees `tx.send(msg).await` succeed as long as the reconnect
  completes before the pipe fills. On permanent failure (max retries
  exhausted), `tx` is closed → caller sees `send() → Err(Closed)`.
```

The caller supplies `Option<Arc<Mutex<PipeSender<WebSocketMessage>>>>` —
`Some(shared_tx)` means "keep this sender stable across reconnects." `None`
means the task creates its own (caller reads `outbound_sender()` to get the
shared handle).

### WsTransport (F39) built on ReconnectingWebSocketTask

With Pipe, F39's `WsTransport` becomes thin glue:

1. `open()` creates a `Pipe<WebSocketMessage>` pair.
2. Spawns `ReconnectingWebSocketTask<SystemDnsResolver>` on the valtron pool.
3. The task gets `PipeReceiver` (outbound) — it drains WS messages from the
   pipe and writes them to TCP as Binary frames.
4. The task's inbound messages surface as `TaskStatus::Ready(WebSocketMessage)`.
   A collector task bridges these into `body_tx` (byte pipe).
5. The caller gets `PipeSender` (for outbound) — it pushes `WebSocketMessage`
   into the pipe and the task picks them up.

No raw TCP, no handshake duplication — the existing proven
`ReconnectingWebSocketTask` handles all of that.

## Scope

- [x] `WebSocketServerTask` with Pipe-based inbound delivery (completed F37 impl — needs Pipe migration)
- [ ] Migrate `WebSocketTask` (client) from `Arc<ConcurrentQueue>` to `PipeReceiver`
- [ ] Migrate `ReconnectingWebSocketTask` to Pipe — stable caller-facing PipeSender across reconnects
- [ ] Delete `WsBytePump` (raw TCP pump in pump.rs) — replaced by ReconnectingWebSocketTask + Pipe
- [ ] WsServerConfig { auto_pong, max_message_size, graceful_close, read_model }
- [ ] Retrofit blocking `WebSocketServerConnection::recv` with the assembler
- [ ] Accept `Option<Arc<dyn EventReadiness + Send + Sync>>` in all three task constructors — caller injects fd readiness (F38)
- [ ] **Spawner**: all three WS task types use `BoxedSendExecutionAction` — never `NoSpawner`. Tasks need spawn capability (drainer during reconnect, collector bridging pipe halves)

## Acceptance criteria

- [x] Multi-frame messages assemble on the server; Ping auto-answered per config; Close handshake completes per config (F37 as committed)
- [ ] `WebSocketTask.next_status()` returns `TaskStatus::Depends(pipe.readiness())` on empty outbound — zero polls on idle connection
- [ ] `WebSocketServerTask` sender returns `TaskStatus::Depends(pipe.vacancy())` on full inbound pipe, wakes on consumer drain
- [ ] `ReconnectingWebSocketTask` reconnects transparently; caller's `PipeSender` survives the reconnect cycle
- [ ] `WsTransport::open()` spawns `ReconnectingWebSocketTask`, returns `TransportStream` with pipe-backed send/recv — no raw TCP
- [ ] Caller-supplied `Option<PipeHalf>` — `Some(half)` uses it, `None` creates internally and exposes via accessor
- [ ] All tasks: `type Spawner = BoxedSendExecutionAction`
