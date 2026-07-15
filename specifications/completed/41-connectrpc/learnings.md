# Spec 41 Learnings

Last updated: 2026-07-10

## F31 — gRPC protocol

### H2Pump Done phase burns worker forever
`TaskStatus::Ready(())` only yields a value — the executor polls again, Done
fires again, forever. The task never completes, its pipes never close, and the
caller blocks on the server's 60s IDLE_TIMEOUT. **Fix**: close pipes + return
`None` (task completion). Measured 28.9M Done polls in one run.

### recv_data_frame drops trailing HEADERS
`Kind::Headers` fell through to `_ => {}` in the draining match. Trailing
HEADERS (`grpc-status`, etc.) were silently consumed by the pump. Needed
`recv_stream_event()` — a unified poll that surfaces both `Kind::Data` and
`Kind::Headers`.

### gRPC unary: request body must be enveloped
`call_unary_post` sent raw bytes. gRPC's `decode_unary_request` expected an
envelope frame. Connect is the identity; gRPC wraps. Added
`encode_unary_request` / `decode_unary_response` to `ProtocolClient`.

### Proto::HTTP11 hardcoded in client
`call_unary_post` always set `Proto::HTTP11` in the request descriptor. gRPC
requires HTTP/2. Fixed by deriving proto from `ProtocolSelection`.

## F36 — WS resumable decoder

### apply_mask must use byte offset
RFC 6455 masking is `payload[i] ^ mask_key[i % 4]`. Applying the mask at the
end (once all payload bytes are accumulated) works because the mask is
position-independent — it doesn't matter *when* you apply it, only that each
byte offset gets the right key byte.

### 64-bit extended length exceeds default fill_from chunk
`AccumulatingBuffer::fill_from` reads 8KB by default. A 70KB payload takes
multiple `step()` calls. The test must loop `step()` → `Pending` → `step()`
until `Frame`.

## F37 — WS server task

### NoSpawner is wrong for valtron tasks
Every valtron task should use `BoxedSendExecutionAction`. Tasks need spawn
capability for sub-tasks (drainer during reconnect, collector bridging pipe
halves). Only the server task had `NoSpawner` — `WebSocketTask` and
`ReconnectingWebSocketTask` were already correct.

### ConcurrentQueue pop() has no wake integration
`pop()` returns `None` on empty — the task must poll again or return
`Delayed`. There's no waker stash. `Pipe::try_recv()` returns `Empty` and the
task can park via `TaskStatus::Depends(pipe.readiness())` — the producer's
`try_send()` fires the waker. Same for `try_send()` → `Full` →
`Depends(pipe.vacancy())`.

## F38 — WS Depends read model

### Caller should inject EventReadiness, not task create it
Making the task extract the fd and register it with a reactor couples the task
to the reactor lifecycle. Instead, the caller wraps the fd (or a timer
fallback) in `Arc<dyn EventReadiness>`, hands it to the task constructor. The
task just calls `is_ready()` and returns `Depends`. Benefits: testable
(inject `AlwaysReady`), portable (caller picks platform-specific impl), no
deregistration leak (caller owns the lifecycle).

### TimerReadiness as reactor fallback
When no kernel reactor is available, `TimerReadiness` implements
`EventReadiness` by returning `true` every N ms. The task code is identical —
it returns `TaskStatus::Depends(timer)` not `TaskStatus::Delayed(Duration)`.
The distinction matters: `Depends` composes with `AnyReadiness`; `Delayed`
is a fixed timeout that doesn't compose.

## F39 — WS transport

### Don't duplicate the WS client stack
Writing a raw TCP pump with manual handshake, frame encode/decode, and
reconnection duplicates what `WebSocketTask` + `ReconnectingWebSocketTask`
already handle correctly. The right approach is thin glue: spawn the existing
task on valtron, bridge its Pipe-based message seam into the standard
`TransportStream` pipes.

### ReconnectingWebSocketTask already surfaces reconnect state
`type Pending = ReconnectingWebSocketProgress` exposes `Connecting`,
`Handshaking`, `Reading`, `Reconnecting`. The transport doesn't need to add
its own status channel — the task already provides it.

### SystemDnsResolver makes the generic concrete
`WebSocketTask<R: DnsResolver>` is generic, but `SystemDnsResolver::default()`
produces a concrete type. `ReconnectingWebSocketTask<SystemDnsResolver>` is
fully monomorphized — it can be spawned via `valtron::send(task)` without
any `Box<dyn TaskIterator>` or trait object.

## General

### The Transport seam is byte-blind — proven by WS
Every protocol handler (`ConnectHandler`, `GrpcHandler`, `GrpcWebHandler`)
operates on `Bytes` through `ByteSink`/`ByteSource` pipes — they never touch
the wire format, DNS, TCP, TLS, or WS frames. Adding `WsTransport` required
zero protocol changes: `encode_unary_request`/`decode_unary_response`
were added for gRPC envelope wrapping (a protocol concern, not transport),
and `TransportStream.trailers` was added because h2 trailing HEADERS are
protocol-relevant metadata — neither was WS-specific. The seam design
from Decision 11 is working as intended.

### Never `loop {}` in `TaskIterator::next_status()`
Every call to `next_status()` must do exactly one step of work and return.
A `loop {}` that drains an unbounded pipe or retries an I/O operation hogs
the worker and starves other tasks. Use bounded drains
(`MAX_OUTBOUND_PER_POLL`) and return `Pending` or `Delayed` when the source
is empty or the sink is full.
