---
feature: "Client-side completion + zero-syscall proxy relay (D14 F4 relay half)"
description: "Outbound-dial completion registration and a readiness-aware splice, so both legs of a proxied connection read from the io_uring inbox and the relay wakes on data instead of a 1 ms sleep — culminating in the SEND_ZC zero-copy handoff"
status: "in-progress (Part A + Part B1 dial-wiring landed + tested; Part B2 readiness-splice + Part C deferred)"
priority: "medium"
phase: 4
depends_on: ["48-transport-completion-read-path", "49-write-side-completion"]
estimated_effort: "large"
created: 2026-07-11
---
# Feature 50: Client-side completion + zero-syscall proxy relay

> **Status: Part A landed (2026-07-12); Parts B and C deferred.**
>
> **Done — Part A (`connect_completion`):** the outbound-dial mirror of
> `accept_connection` is implemented in `foundation_iogate::native::accept` and
> exported as `foundation_iogate::connect_completion(addr, mode)`. It dials, sets
> the socket non-blocking, registers it with the shared reactor, and returns a
> `Connection::Completion` (or a plain `Connection::Tcp` for `ServerIo::Std`) —
> reusing the same `CompletionSocket` + token allocator as the accept path, so an
> upstream leg reads from the io_uring inbox. Verified by
> `connect_completion_dials_a_completion_backed_upstream` and the `Std` variant in
> `iogate_tests.rs`.
>
> **Done — Part B1 (proxy dial wiring, 2026-07-12):** `foundation_proxy` gained a
> direct `foundation_iogate` dep and an `io_mode: ServerIo` config knob
> (`ProxyConfig::io_mode`, default `Std`), threaded through `ProxyState` →
> `relay` → `tunnel_tcp`/`forward_upgrade`, both of which now dial the upstream via
> `iogate::connect_completion(addr, io_mode)`. In `Completion` mode the upstream
> leg reads from the io_uring inbox and writes via `IORING_OP_SEND` (F49); in `Std`
> mode it is a plain non-blocking socket, so the splice is byte-for-byte unchanged.
> `connect_completion` now always sets the dialed socket non-blocking. The upstream
> mode tracks the front-end mode (`ServerConfig::with_io(config.io_mode)`). The
> upgrade path got a `WouldBlock`-tolerant head write (`Completion`'s `flush` parks
> until the SEND CQE). Verified: all 41 proxy tests pass on the `Std` default (no
> behaviour change); `connect_completion` itself is covered in `iogate_tests.rs`.
> Also migrated the proxy + `foundation_db` off the deprecated `SimpleHttpClient`
> alias while here.
>
> **Blocked — Part B2 (readiness-aware splice): the design's "block the thread on
> `CompositeReadiness`" is not implementable as written.** Investigation
> (2026-07-12) found that `EventReadiness::is_ready(dur)` is **non-blocking** for fd
> readiness: `FdRegistration::is_ready` and `SharedReadiness::is_ready` both *ignore*
> the `dur` argument and return the current latch state
> (`reactor.is_ready(token)`). `CompositeReadiness::is_ready` just ORs the two, so it
> is non-blocking too. There is **no condvar/park primitive** that blocks a raw
> thread until an fd becomes ready — blocking-until-ready exists only inside the
> reactor's `poll()` loop and is consumed by valtron tasks through
> `TaskStatus::Depends`. So parking the splice thread on a composite readiness would
> **busy-spin**, not sleep.
>
> B2 therefore needs one of two larger changes, both flagged in this doc's
> concurrency-model open question:
> 1. **A new blocking-readiness primitive** — e.g. a `Condvar` on `SharedReadiness`
>    that the reactor's wake path notifies. This touches the reactor's hot wake path
>    and every readiness consumer (high blast radius, correctness surface: lost
>    wakeups, spurious wakeups).
> 2. **Convert the splice to a valtron task** returning `Depends(CompositeReadiness)`
>    — but the passthrough is *deliberately* on a dedicated OS thread (see
>    `handler.rs`: "keeps upstream I/O off the valtron worker pool… doing that from a
>    pool worker risks a cross-executor stall"), so this reverses an explicit design
>    choice.
>
> Part B1 already delivered the syscall-removal (upstream reads from the inbox,
> writes via SEND). B2 is only the *latency-floor* removal, and it is gated on
> picking one of the two primitives above — a design decision with real trade-offs,
> not a mechanical change. Deferred pending that decision.
>
> **Deferred — Part C (SEND_ZC zero-copy relay):** F49's SEND pool now exists, but
> `IORING_OP_SEND_ZC` has distinct two-notification completion semantics and needs
> the Part B splice first.
>
> The original design proposal follows, retained for context.

## Why this exists

Feature 48 gave the **server accept** path a completion-mode byte source: an
inbound connection accepted through `foundation_iogate::accept_connection` reads
from the io_uring inbox with zero `read(2)` syscalls. That is half of a proxy.

A proxy relays **between two connections**. Its hot path
(`foundation_proxy::passthrough::splice_bidirectional`) copies bytes both ways
until one side closes. Today, even with the front-door in `ServerIo::Completion`:

1. The **upstream leg is an outbound `TcpStream::connect`** (`tunnel_tcp`,
   `forward_upgrade`, and `SimpleHttpClient` for `forward_http`). It is never
   registered with the reactor, so its reads are `read(2)` and there is no inbox
   to park on.
2. `splice_bidirectional` runs on a **dedicated OS thread** and **sleeps 1 ms**
   whenever neither direction had data — a latency floor that interactive raw
   passthrough (RDP, VNC, noVNC, post-`101` WebSocket relay) pays on every idle
   round trip.

Note what is **not** broken. Copying between a completion-backed `conn` and a
plain `upstream` is already *correct*: splice reads `conn` through its
`Connection::read → read_bytes` stack, which pops the kernel inbox, and writes to
`upstream` with `write(2)`. `read_bytes` is the "two implementations, one code
path" abstraction from F48 — the caller cannot tell an inbox pop from a `read(2)`,
so no bytes are lost and the RECV-drains-the-socket hazard never bites (splice
never issues a raw `read` on the completion fd, and never `split`s it). This
feature is a **performance** feature: it removes the 1 ms floor and the
upstream-leg syscalls. It changes no observable bytes.

## The question this design has to answer

> *"Copying between two connections works when one is io_uring. So what is left?"*

Three things, in increasing cost:

1. **Register the outbound dial** so the upstream leg also reads from an inbox.
2. **Park the splice on a composite readiness** over both inboxes instead of
   sleeping — which forces a decision about the splice's concurrency model.
3. **Hand the RECV buffer straight to a SEND** (`IORING_OP_SEND_ZC`) so bytes
   move `fd → ring → fd` without a user-memory `memcpy` — the true zero-copy
   relay, which needs Feature 49's SEND machinery first.

## The architectural wrinkle: the client dial lives *below* iogate

F48's server accept path was easy to seat: it lives in `foundation_http`, which
is free to depend on `foundation_iogate`. The **client dial is not**. It lives in
two places with different constraints:

- **`foundation_proxy`** dials its raw-passthrough and upgrade upstreams directly
  (`std::net::TcpStream::connect(&authority)` in `tunnel_tcp` / `forward_upgrade`).
  The proxy already depends on `foundation_http → foundation_iogate`, so it can
  call an iogate client-dial helper directly. **This leg is easy.**
- **`foundation_netio::simple_http::client`** dials the upstream for the plain
  `forward_http` path (`SimpleHttpClient::send → RawStream::from_connection`).
  `foundation_netio` **cannot depend on `foundation_iogate`** — that would close
  the cycle `iogate → netio → iogate`. So netio's client cannot *call* the
  completion registration; it must be *handed* a completion-backed `Connection`
  from above, exactly as `Connection::Completion(Box<dyn CompletionReadWrite>)`
  lets the server side inject one (F48). **This leg needs a construction seam.**

This split is the spine of the phased rollout: the raw splice (proxy-dialed) can
land first; the `SimpleHttpClient` path needs an injection seam and lands second.

## The design

### Part A — outbound completion dial in `foundation_iogate`

A client mirror of `accept_connection`, in `foundation_iogate::native`:

```rust
/// Dial `addr`, register the connected socket with the shared reactor, and arm
/// completion mode where the backend allows it. The client analogue of
/// `accept_connection`; the returned Connection reads from the io_uring inbox.
pub fn connect_completion(addr: SocketAddr, mode: ServerIo) -> io::Result<Connection>;
```

`ServerIo::Std` returns a plain `Connection::Tcp`; the other modes register and
return `Connection::Completion(Box::new(CompletionSocket::completion(..)))` —
reusing the *same* `CompletionSocket` and `Token` allocator F48 already built.
RECV on a freshly connected client socket is as valid as on an accepted one; the
only difference from the accept path is who owns the `connect`.

`foundation_proxy::tunnel_tcp` and `forward_upgrade` swap their
`TcpStream::connect` for `iogate::connect_completion(addr, mode)` and hand the
resulting `Connection` to the splice. Both legs are now inbox-backed.

### Part B — a readiness accessor and the readiness-aware splice

`splice_bidirectional<A: Read + Write, B: Read + Write>` exposes no readiness
handle. Add an **optional** capability trait so the generic stays intact:

```rust
/// A stream that can hand back the reactor readiness a task parks on. Completion
/// and readiness-registered connections implement it; a plain socket does not.
pub trait ReadinessSource {
    fn readiness(&self) -> Option<EventReadinessPtr>;
}
```

`CompletionSocket` implements it (from `self.inner().registration`); so does the
`Connection::Completion` arm; a plain `Connection::Tcp` returns `None`. The splice
then has two modes:

- **Both sides yield a readiness** → build a `CompositeReadiness` over the two
  and block on it (with a timeout for the shutdown check) instead of sleeping.
  When either inbox has bytes, the wait returns and the copy proceeds.
- **Either side is `None`** (a plain socket, or a non-completion backend) → the
  existing 1 ms sleep loop, unchanged. The feature degrades to today's behaviour.

`foundation_nativeapis` already exports `CompositeReadiness`; this reuses it.

#### The concurrency-model decision

`splice_bidirectional` runs on a dedicated OS thread. Two ways to park it:

| Option | Shape | Cost |
|---|---|---|
| **Block the thread on `CompositeReadiness`** | Keep the thread; replace `sleep(1ms)` with a bounded `wait` on the composite readiness | Minimal blast radius; one thread per relay stays | 
| **Convert to a valtron task** | The relay returns `Depends(CompositeReadiness)` and is re-polled; no dedicated thread | Frees the thread, matches the reactor model — but the passthrough listener and its lifecycle move onto valtron |

Recommendation: **block the thread** for the first cut (smallest change, keeps the
passthrough's independent lifecycle), and evaluate the valtron-task conversion
only if per-relay threads become a scaling problem. The `wait` still needs a
timeout so `OnSignal` shutdown is observed within a bounded window (today it is
≤1 ms; keep that guarantee).

### Part C — the zero-copy relay (needs Feature 49)

Parts A and B still `memcpy`: kernel → RECV ring → user buffer → `write(2)`. The
final step is Decision 30's Phase 4 and F49's deferred "Phase D": read a
`ProvidedBuf` from the RECV ring on one fd and `IORING_OP_SEND_ZC` it on the
other, so bytes never touch user memory. This **requires F49's SEND pool and
`send_bytes`**, and turns `splice_bidirectional` into a buffer-ring handoff
rather than a copy loop. It is the last phase here and is gated on F49 landing.

## What this affects (blast radius)

- **`foundation_iogate`**: new `connect_completion` (Part A); a `ReadinessSource`
  trait + impls on `CompletionSocket` and the `Connection::Completion` arm
  (Part B). Reuses the existing `CompletionSocket`, token allocator, and reactor
  init — no new registration machinery.
- **`foundation_netio`**: a construction seam on `SimpleHttpClient` so a caller
  can inject a pre-built `Connection` (or a boxed dial factory) instead of the
  client always calling `TcpStream::connect` itself. netio gains **no** iogate
  dependency — the completion-backed `Connection` arrives as the existing
  `Box<dyn CompletionReadWrite>`. This is the larger, second-phase change.
- **`foundation_proxy`**: `tunnel_tcp` / `forward_upgrade` dial via iogate;
  `splice_bidirectional` gains the readiness-aware path (Part B) and, later, the
  SEND_ZC handoff (Part C). The concurrency-model choice lives here.
- **Buffer pool**: N concurrent relays share F48's single 256 × 16 KiB RECV ring.
  A slow relay holding `ProvidedBuf`s starves the others (F48 open question #3,
  arriving from the relay direction). Sizing — one pool per reactor, per worker,
  or per listener — is an open question this feature must answer before Part C.
- **No change**: `RawStream`, `Connection`'s public shape, the decoder, the
  server accept path, and every wasm build. Bytes on the wire are identical.

## Scope

- `foundation_iogate::connect_completion` — the outbound-dial mirror of
  `accept_connection`.
- `ReadinessSource` trait + impls; the readiness-aware `splice_bidirectional`
  with graceful fallback to the sleep loop.
- `foundation_proxy` wiring: iogate-dialed upstreams; the parked splice; the
  thread-vs-valtron concurrency decision.
- `SimpleHttpClient` injection seam for a completion-backed upstream `Connection`.
- Part C (SEND_ZC relay) — gated on Feature 49.

## Phased rollout

- **Phase 1: outbound dial + readiness-aware splice for proxy-dialed upstreams.**
  `connect_completion`, `ReadinessSource`, the parked splice with sleep fallback,
  and `tunnel_tcp` / `forward_upgrade` wiring. Removes the 1 ms floor and the
  upstream-leg syscalls for raw passthrough and WS relay. No F49 dependency.
- **Phase 2: `SimpleHttpClient` completion upstream.** The injection seam so the
  plain `forward_http` path also reads its upstream from an inbox.
- **Phase 3: buffer-pool contention.** Decide pool topology under many relays;
  implement whatever Part C needs (F48 OQ#3).
- **Phase 4 (needs F49): the SEND_ZC zero-copy relay.** RECV `ProvidedBuf` →
  `IORING_OP_SEND_ZC`, no user-memory copy.

## Verification

- `cargo test -p foundation_proxy` — byte parity of a spliced connection with the
  readiness-aware path vs the sleep path (identical bytes both ways).
- The ptrace syscall counter (`uring_completion_syscall_tests.rs`) pointed at a
  raw passthrough: **zero** read-family syscalls on *both* legs, epoll as control.
- A latency probe: p99 round-trip on an idle-then-active passthrough, readiness
  path vs the 1 ms sleep path — the sleep floor should disappear.
- `wrk`/`iperf`-style throughput through the relay, io_uring vs epoll.

## Out of scope

- Server accept-path completion — shipped in Feature 48.
- The SEND mechanism itself (`SendPool`, `send_bytes`) — Feature 49; this feature
  consumes it in Part C but does not build it.
- HTTP/3 / QUIC relays — separate UDP path.
- Per-worker rings as a default — evaluated in Phase 3, not assumed.

## Open questions for review

1. **Client dial ownership.** Should `SimpleHttpClient` take an injected
   `Connection`, or a boxed `dial: Fn(SocketAddr) -> io::Result<Connection>`
   factory? The factory keeps retry/redirect logic (which re-dials) inside netio;
   the injected connection does not survive a redirect. Redirect handling
   (`request_redirect`) likely forces the factory.
2. **Thread vs valtron task for the splice.** Blocking the thread is the smaller
   change; the valtron conversion frees the thread but moves the passthrough
   lifecycle. Which wins under thousands of idle relays?
3. **Buffer-pool topology.** One shared RECV ring across all relays invites
   head-of-line starvation. Pool per reactor, per worker, or per listener — and
   does Part C's SEND_ZC need its own pool discipline?
4. **Composite readiness wait timeout.** Today shutdown is observed within ≤1 ms
   (the sleep). The parked wait needs a timeout to preserve that; what value
   trades shutdown latency against wakeups on a fully idle relay?

## Acceptance criteria

- A raw `tcp://` passthrough and a post-`101` WebSocket relay, both legs on a
  completion-capable kernel, complete a bidirectional exchange with **zero
  read-family syscalls on either socket**, measured by the ptrace counter.
- On an idle-then-active passthrough, the readiness path shows **no 1 ms latency
  floor**; the epoll control arm still works via the sleep fallback.
- A plain socket (or epoll backend) on either leg transparently falls back to the
  sleep loop — the splice stays correct and generic.
- Bytes relayed are identical to today's splice; no protocol is altered.
- Part C, when F49 has landed: the relay moves bytes with no user-memory
  `memcpy`, verified by the buffer-ring handoff and unchanged throughput at lower
  CPU.
