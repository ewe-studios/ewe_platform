---
feature: "Connection-owner walking skeleton — end-to-end spine over a real socket (D11 §Connection ownership)"
description: "Server per-connection pump (Serve adapter) + client open() pump + one unary and one streaming RPC over loopback TCP; lands PushableRequestBody::into_sender"
status: "in-progress"
priority: "critical"
phase: 1
depends_on: ["17-transport-seam", "22-router-dispatch", "07-pushable-request-body", "10-reactor-parking"]
estimated_effort: "large"
created: 2026-07-05
---
# Feature 44-connection-owner-walking-skeleton: end-to-end spine over a real socket

## Why this exists (sequencing correction)

Features 17–22 built the RPC middle (seam, protocols, router) as isolated units tested against
**stubbed** connection owners (F22's `feeder`/`collector`, in-memory `block_on`). The **connection
owner** — the component holding the raw fd that spawns the byte pump owning the socket-facing pipe
halves — was never laid down as a first, load-bearing spine, so the ownership contract kept
resurfacing as ambiguity at each layer boundary (most sharply in F23's `open`). This feature builds
the thin vertical slice that makes the Decision 11 §Connection ownership contract concrete, so
F23/F24 and every later transport slot into a **proven** path.

## Normative sources (read before writing code)

- decisions/11-transport-seam.md — §Connection ownership (both sides), §Duplex table, §who-owns-enveloping
- decisions/08-router-and-dispatch.md — §Integration with foundation_http, Decided Detail 0 (server connection owner)
- decisions/07-client-architecture.md — Decided Details (connection ownership / reuse)

## Scope

- **netio addition:** `PushableRequestBody::into_sender(self) -> PipeSender<Bytes>` so the pushable
  body's pipe *is* the seam `send_body` (no bridge task, no copy).
- **Server connection owner:** a `Serve`/`ServeWriter` adapter for `ConnectRpcHandler`. `serve()`
  owns the `RawStream`, creates the byte pipes, **spawns the byte pump** holding the socket-facing
  halves (read fd → response... wait: server pump = read fd → `request` byte-pipe sender; drain
  `response` byte-pipe receiver → write fd), parked on the nativeapis reactor; invokes dispatch
  with the caller-facing halves; streams the response back. Replaces F22's `feeder`/`collector`.
- **Client connection owner:** `Transport::open` over the netio h1 client spawns its pump
  (driven `ClientRequest::send_async`) per Decision 11; returns `TransportStream`.
- **One walking test each:** a unary RPC and a server-streaming RPC driven **over a real loopback
  TCP socket**, server accept→dispatch→respond and client connect→send→receive, asserting the
  decoded round-trip (not an in-memory pipe pump).

## Out of scope

- Full client core ergonomics (F24), auth (F25), codegen (F26/27), HTTP/2+ (F29+).
- Pool-reuse assertion detail and Fetch capabilities (stay in F23).

## Acceptance criteria

- `into_sender` lands; netio lib clean; existing pushable-body tests still pass.
- Server `Serve` adapter: a per-connection task owns the fd and pumps; dispatch runs with the
  caller-facing halves; response streams back through the pump (no full-response buffering for the
  streaming case).
- Client `open`: pump spawned before return; a request larger than the pipe depth does **not**
  deadlock (regression guard for the lazy-drive trap).
- End-to-end: unary + server-streaming RPC complete over a real loopback socket; `#[valtron_test]`,
  `--profile uat`.
