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

### Where this sits in the stack

This feature changes the **internal queuing** inside `WebSocketTask`,
`WebSocketServerTask`, `ReconnectingWebSocketTask`, and `MessageDelivery`.
It does NOT touch the protocol layer (`ProtocolHandler`, `Router`, `Client`,
codec, compression, envelope, interceptor, auth). Those layers never see
`ConcurrentQueue` or `Pipe` — they only see `ByteSink`/`ByteSource` on
`TransportStream`. The WS message queue is a transport-internal concern.

```
Protocol layer:      Bytes in, Bytes out on TransportStream pipes
                           │
Transport seam:      ByteSink / ByteSource / HeadStream / trailers
                           │
WsTransport:         MessageDelivery (PipeSender) + WebSocketClient stream
                           │
WS tasks:            Pipe<WebSocketMessage>   ← what this feature migrates
                           │
TCP:                 WS Binary frames on the wire
```

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

`Pipe<T>` (F02) wraps a bounded `ConcurrentQueue<T>` and adds:

- Two single-slot **waker stashes** (consumer waker + producer waker)
- `QueueReadiness` — `EventReadiness` impl; ready when the queue is non-empty
- `QueueVacancyReadiness` — `EventReadiness` impl; ready when the queue has capacity
- `try_send`/`try_recv` — non-blocking for the valtron task path; return `Empty`/`Full` so the task can park via `TaskStatus::Depends(pipe.readiness())`
- `send().await`/`receive().await` — async, park the future on the waker stash; producer/consumer wake each other
- Cancel composition via `AnyReadiness` — `send().await`/`receive().await` compose the caller's `CancelSignal`

Same underlying queue. Nicer surface for both sync (task) and async (future) callers. Already used by `FramePipe`, `ByteSink`, `ByteSource`, `send_body`, `recv_body`, `head_rx`, and `trailers` — every other seam in the transport stack.

**`MessageDelivery`** is the existing public wrapper in `connection.rs`. After migration:

```
Before: queue: Arc<ConcurrentQueue<WebSocketMessage>>
After:  tx: PipeSender<WebSocketMessage>   // same underlying queue + wakers

Public API unchanged: send(msg), ping(), pong(), close() — same call sites.
New: pipe() → &PipeSender for Transport bridging, send_async().await for async code,
     into_pipe() for consuming conversion.

WebSocketClient::connect() still returns (client, MessageDelivery) — zero API breakage.
```

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

#### `MessageDelivery` wraps `PipeSender` — same API, Pipe-backed

The existing `MessageDelivery` type in `connection.rs` wraps
`Arc<ConcurrentQueue<WebSocketMessage>>`. Users push messages through it,
the task drains from the other side. After migration, the inner type becomes
a `PipeSender<WebSocketMessage>`:

```
Before:
  queue: Arc<ConcurrentQueue<WebSocketMessage>>
  send(msg) → queue.push(msg)         // fails with ConnectionClosed if full
  queue() → &Arc<ConcurrentQueue<...>> // raw access for the task side

After:
  tx: PipeSender<WebSocketMessage>
  send(msg) → tx.try_send(msg)        // task path: Ok | Full → Depends(vacancy)
  send_async(msg) → tx.send(msg).await // async path: parks on backpressure
  pipe() → &PipeSender<WebSocketMessage>  // raw pipe access
  queue() → &Arc<ConcurrentQueue<...>>    // deprecated, kept for transition
  close() → drop(tx)                     // receiver side sees Closed/None
```

The public API is nearly identical. Users who called `delivery.send(msg)`
continue to do so. Users who need the raw pipe (e.g. `WsTransport::open()`
bridging into `body_tx`) call `delivery.pipe()`.

#### Pipe ownership model — caller-supplied or internal

Every task that uses a `Pipe` takes an `Option<PipeHalf>` — if the caller
passed `Some(half)`, the task uses it; if `None`, the task creates a fresh
`Pipe` pair and **exposes the other half** via `MessageDelivery`:

```
// Caller creates the Pipe, keeps tx, gives rx to the task:
let (tx, rx) = Pipe::<WebSocketMessage>::with_depth(16);
let delivery = MessageDelivery::from_pipe(tx);  // caller holds delivery
let task = WebSocketTask::connect_with_pipe(resolver, url, rx)?;

// OR: Task creates the Pipe, exposes delivery handle:
let (client, delivery) = WebSocketClient::connect(resolver, url)?;
// `delivery` wraps the PipeSender — same API as today

// OR: Internal task constructor, caller reads delivery:
let task = WebSocketServerTask::new(stream, config, None);
let delivery = task.delivery();  // returns &MessageDelivery
```

This preserves the current `WebSocketClient::connect() → (Self, MessageDelivery)`
return signature — zero API breakage for existing callers. The internal change
from `ConcurrentQueue` to `Pipe` is transparent to them, except they now get
wake integration and backpressure for free.

#### WebSocketServerTask

```
Before:
  task pushes into Arc<ConcurrentQueue<WebSocketMessage>>
  caller drains via delivery.queue().pop()

After:
  task owns PipeSender<WebSocketMessage> (inbound, task→caller)
  caller holds MessageDelivery wrapping the PipeSender

  task.next_status():
    assembles message → delivery.try_send(msg)
      Ok → done.
      Full → TaskStatus::Depends(delivery.vacancy())  // parks until consumer drains
    on Close → drop(delivery)                         // caller's rx.try_recv() → Closed
```

#### WebSocketClient (wrapper, not a task)

`WebSocketClient` wraps `WebSocketTask` behind a `DrivenStreamIterator` and
provides a simple `next()` → `Stream<WebSocketMessage>` iterator. After F37:

- **Internally**: `WebSocketClient::connect()` still creates a `WebSocketTask`,
  spawns it via `execute(task, None)`, and returns `(Self, MessageDelivery)`.
  The only internal change is that `MessageDelivery` now wraps a `PipeSender`
  instead of `Arc<ConcurrentQueue>`.
- **Public API**: unchanged. `client.next()` still yields inbound messages.
  `delivery.send(msg)` still pushes outbound messages. Users see zero
  difference — they get wake integration and backpressure for free.
- **WsTransport**: calls `WebSocketClient::connect()`, spawns a collector
  bridging the client stream into `body_tx`, and uses `delivery.into_pipe()`
  as `TransportStream::send_body`. ~30 lines of glue.

One connection = one task. No reconnection. Migration:

```
Before:
  task pops from Arc<ConcurrentQueue<WebSocketMessage>>
  caller pushes via delivery.send(msg)

After:
  task owns PipeReceiver<WebSocketMessage> (outbound, caller→task)
  caller holds MessageDelivery wrapping the PipeSender

  WebSocketClient::connect() returns (Self, MessageDelivery) — same API,
  same convenience methods (send, send_text, close, etc.), now Pipe-backed.

  task.next_status():
    match rx.try_recv() {
      msg → write WS Binary frame to TCP
      Empty → TaskStatus::Depends(pipe.readiness())  // parks on QueueReadiness
      Closed → drain + close TCP (caller dropped delivery)
    }
```

`MessageDelivery::send()` calls `tx.try_send(msg)`. On `Full`, the task
parks via `Depends(delivery.vacancy())`. The caller can also use
`delivery.send_async(msg).await` — the async path parks via the pipe's
producer waker. Either way, zero wasted polls.

#### ReconnectingWebSocketTask — wraps WebSocketTask, adds reconnection

Layers reconnection over the single-shot `WebSocketTask`. On disconnect:
exponential backoff → new `WebSocketTask` → resume. The caller sees a
single stable task that never completes unless the retry limit is hit.

Already surfaces connection state via `type Pending = ReconnectingWebSocketProgress`:
`Connecting` / `Handshaking` / `Reading` / `Reconnecting`. Callers that want to
expose reconnect status already have it through the task's pending messages.

After Pipe migration, the reconnect cycle preserves the caller's Pipe ends:

```
Before:
  Each reconnect creates a new inner WebSocketTask with a new queue.
  The caller's old queue is orphaned; messages sent during reconnect are lost.

After:
  The caller holds the SAME PipeSender<WebSocketMessage> across reconnects.
  ReconnectingWebSocketTask stores:
    outbound_tx: Arc<Mutex<PipeSender<WebSocketMessage>>>  // shared with caller
    outbound_rx: Option<PipeReceiver<WebSocketMessage>>    // current inner task's rx

  On reconnect:
    1. Create new Pipe pair.
    2. Swap shared_tx to point at the new pipe.
    3. Spawn drainer (via BoxedSendExecutionAction): forward remaining messages
       from old rx into new tx.
    4. Give new rx to the fresh inner WebSocketTask.
    5. Drop old inner task.

  Caller sees tx.send(msg).await succeed across reconnect cycles. On permanent
  failure → tx closes → caller sees send() → Err(Closed).
```

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

- [x] `WebSocketServerTask` with Pipe-based inbound delivery (committed F37 impl — needs Pipe migration)
- [ ] Migrate `MessageDelivery`: replace `Arc<ConcurrentQueue<WebSocketMessage>>` with `PipeSender<WebSocketMessage>`:
  - `send(msg)` → `self.tx.try_send(msg)` (existing sync API preserved)
  - `send_async(msg)` → `self.tx.send(msg).await` (new async API — parks on backpressure)
  - `pipe()` → `&PipeSender<WebSocketMessage>` (new — raw pipe access for Transport bridging)
  - `queue()` → `&Arc<ConcurrentQueue<...>>` (deprecated, kept for transition)
  - `close()` → `drop(self.tx)` (receiver side sees `Closed`)
  - `into_pipe()` → `PipeSender<WebSocketMessage>` (consuming conversion for Transport::open)
- [ ] Migrate `WebSocketTask` (client) from `Arc<ConcurrentQueue>` to `PipeReceiver`
- [ ] Update `WebSocketClient::connect()` internally — still returns `(Self, MessageDelivery)`, same API, now Pipe-backed
- [ ] Migrate `ReconnectingWebSocketTask` to Pipe — shared `Arc<Mutex<PipeSender>>` across reconnects
- [ ] Delete `WsBytePump` (raw TCP pump in pump.rs)
- [ ] WsServerConfig { auto_pong, max_message_size, graceful_close, read_model }
- [ ] Retrofit blocking `WebSocketServerConnection::recv` with the assembler
- [ ] Accept `Option<Arc<dyn EventReadiness + Send + Sync>>` in all three task constructors — caller injects fd readiness (F38)
- [ ] Fix `WebSocketServerTask::Spawner = BoxedSendExecutionAction` (others already correct)
- [ ] Update existing `WebSocketClient` tests in `tests/websocket/` — verify `delivery.send()` + `client.next()` work same as before

## Acceptance criteria

- [x] Multi-frame messages assemble on the server; Ping auto-answered per config; Close handshake completes per config (F37 as committed)
- [ ] `WebSocketTask.next_status()` returns `TaskStatus::Depends(pipe.readiness())` on empty outbound — zero polls on idle connection
- [ ] `WebSocketServerTask` sender returns `TaskStatus::Depends(pipe.vacancy())` on full inbound pipe, wakes on consumer drain
- [ ] `ReconnectingWebSocketTask` reconnects transparently; caller's `PipeSender` survives the reconnect cycle
- [ ] `WsTransport::open()` spawns `ReconnectingWebSocketTask`, returns `TransportStream` with pipe-backed send/recv — no raw TCP
- [ ] `ReconnectingWebSocketProgress` (already `type Pending`) surfaces reconnect state — callers observe `Connecting`/`Handshaking`/`Reading`/`Reconnecting` without polling internals
- [ ] Caller-supplied `Option<PipeHalf>` — `Some(half)` uses it, `None` creates internally and exposes via accessor
- [ ] `WebSocketServerTask::Spawner` fixed to `BoxedSendExecutionAction` (`WebSocketTask` + `ReconnectingWebSocketTask` already use it)
