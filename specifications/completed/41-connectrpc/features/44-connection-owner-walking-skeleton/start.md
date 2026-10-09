---
spec_directory: "specifications/41-connectrpc"
feature: "44-connection-owner-walking-skeleton"
created: 2026-07-05
updated: 2026-07-05
---

# Start: Connection-owner walking skeleton — end-to-end spine over a real socket

1. Read `plan.md` (esp. the fifth-pass connection-ownership note), then every doc under Normative
   sources in `feature.md` — the decisions are normative; this feature file is the work unit.
2. Read `.agents/skills/rust-clean-code/` and `rust-valtron-usage` (this is pool/reactor work).
3. Land `PushableRequestBody::into_sender` in foundation_netio first (small, verify pushable tests). ✅ DONE
4. Build the **server** `Serve` adapter (`ConnectRpcServe`) — connection-owner pump per
   Decision 11 §Connection ownership. ✅ DONE
5. Build the **client** `Transport` adapter (`H1Transport`) — valtron-native: `open()` spawns the
   pump via `valtron::send()` and returns the three caller-facing `Pipe` halves synchronously.
   No `BoxFuture`, no `futures_lite::block_on`, no OS threads — the pump is a valtron task that
   calls `ClientRequest::send_async()`, feeds `head` and `recv_body` pipes. The caller is a
   valtron task (or uses the pipe's async surface). Needs:
   - `Transport::open() -> Result<TransportStream, TransportError>` (sync return, was `BoxFuture`)
   - `TransportStream.head: PipeReceiver<(Status, SimpleHeaders)>` (was `response: BoxFuture`)
   - Pump task spawned via `valtron::send()`; drives `send_async()` internally
6. Prove it with real loopback-socket tests (unary + server-stream + H1Transport client round-trip).
   `#[valtron_test]` + `--profile uat` + `--features multi`.
7. Verify every Acceptance criterion, then update `feature.md` status.
