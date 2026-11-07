---
feature: "Send the gRPC status over h3 and send handler streaming headers"
status: "pending"
priority: "high"
depends_on: []
crates: ["foundation_connectrpc"]
issues: [253, 258]
model: "sonnet"
branch: "spec73/f03-connectrpc-h3-trailers-and-headers"
---

# Feature 03: h3 trailers and handler streaming headers

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Google Remote Procedure Call (gRPC); HTTP/3 (h3); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Close [#253](https://github.com/ewe-studios/ewe_platform/issues/253) (h3 sends no
`grpc-status`) and [#258](https://github.com/ewe-studios/ewe_platform/issues/258) (handler
streaming headers are never sent).

## 2. Problem (measured on master, 2026-11-07)

1. **#253 — already fixed. Close it with evidence.** `native/h3_serve.rs:573`
   `send_h3_response` takes `response.trailers` at line 577 and writes a HEADERS trailer frame
   at lines 595-598. `tests/h3_streaming_tests.rs` already asserts a `grpc-status` trailer for
   a unary call (line 514) and a streaming call (line 537). The feature closes the issue and
   adds no code for it.
2. **#258 — real.** The carrier moved. `MessageSink::set_headers`
   (`shared/transport/facade.rs:109`) calls `ConnSender::send_headers`, which stores the map in
   `PipeConnSender.response_headers` (`shared/transport/conn.rs:242-252`). That slot is
   `HandlerConnEnds.response_headers` (`conn.rs:121`, shared `Arc<Mutex<...>>`). **No transport
   reads it.** All three pumps build the response head themselves from only `content-type`
   and the streaming encoding:
   - h1 and the Connect/gRPC POST path: `shared/router/dispatch.rs:801-824` (`blank_response`
     plus content type and encoding, then `sink.open(head)` on the first chunk);
   - h2: `shared/router/dispatch.rs:1749-1770` (the h2 stream pump, same shape);
   - h3: `native/h3_serve.rs:299-320` (`pump_content_type` plus optional encoding only).
   So a header a handler sets is stored and dropped. A second defect is in the same path:
   `send_headers` after the first frame returns `Ok` and silently discards the map
   (`headers_sent` already true at `conn.rs:242`).

## 3. Requirements

1. A gRPC stream over h3 ends with a trailer section that holds `grpc-status` and
   `grpc-message`, exactly as h2 and h1 do. A client over h3 reads a `Code` for every
   outcome, including an error.
2. Headers a handler sets before its first message are sent as the response head on h1, h2
   and h3. Headers set after the head are trailing metadata, or a typed error if the
   transport cannot send them. State the rule in the docs.
3. One shared function renders the trailer section for all three transports. A change to the
   trailer rules happens in one place.
4. Stream bodies; do not collect them (root rule 4).
5. No public API change unless a new item is required. Apply the version-bump rule then.

## 4. Design (completed 2026-11-07)

### 4.1 Carry the handler header map to the pump

`HandlerExchange` (`shared/protocol/mod.rs:57`) carries `conn`, `reader_task` and
`writer_task`. Add one field:
```rust
pub struct HandlerExchange {
    pub conn: Box<dyn HandlerConn>,
    pub reader_task: BoxedTask,
    pub writer_task: BoxedTask,
    /// The map a handler installs with `MessageSink::set_headers`, shared with
    /// the conn's sender. The pump merges it into the response head.
    pub response_headers: Arc<Mutex<Option<SimpleHeaders>>>,
}
```
Each protocol `new_conn` fills it from the `PipeHandlerConn::new` ends. This is a public API
change: bump `foundation_connectrpc` and every consumer requirement.

### 4.2 One merge helper

Add `fn take_handler_headers(slot: &Arc<Mutex<Option<SimpleHeaders>>>) -> SimpleHeaders`
beside the head builders. It replaces the slot with `None` and returns the map, so one head
gets one map. In each pump, call it when the head is built (before the first chunk):
- h1/POST: insert every entry into `head.headers` **after** the content type and encoding
  insert, so a handler cannot silently change the wire framing headers. If the handler sets
  `content-type` or the encoding header, keep the protocol value and log the discard.
- h2 and h3: extend the header list built in the pump, under the same rule.

If the pump never gets a first chunk (handler failed early), the merged headers go with the
error response the existing code already builds.

### 4.3 A late header is an error

In `PipeConnSender::send_headers` (`conn.rs:242`), when `headers_sent` is already true,
return `Code::FailedPrecondition` with the message
`"response headers must be set before the first message"`. `MessageSink::set_headers` already
propagates the error, so a handler sees it. Document the rule on `MessageSink::set_headers`:
before the first message the map is the response head; after it, the call fails.

### 4.4 #253

No code. Close it in the PR with the lines from section 2 and the two existing tests as the
evidence.

## 5. Tasks

- [ ] Verify #253 and #258 on master. Record the evidence (done above).
- [ ] Add `response_headers` to `HandlerExchange` and fill it in all three protocols.
- [ ] Add `take_handler_headers` and merge in the h1/POST, h2 and h3 pumps.
- [ ] Return `FailedPrecondition` for a late `set_headers`.
- [ ] Add the regression tests.
- [ ] Bump `foundation_connectrpc` and every consumer.
- [ ] Run the verify commands. Record the results.
- [ ] Close #253 (evidence) and #258 (fix) in the PR body.

## 6. Tests

Tests live in `backends/foundation_connectrpc/tests/` as top-level integration test files.
The crate has no `tests/mod.rs`; Cargo discovers each file.

1. `handler_headers_reach_the_response_head_h1`, `_h2`, `_h3`: a streaming handler calls
   `set_headers` with `x-handler: one` before its first message; assert the response head on
   each transport carries it. Old code: absent.
2. `a_handler_cannot_override_content_type`: the handler sets `content-type`; assert the
   protocol value wins and the call succeeds.
3. `set_headers_after_the_first_message_fails`: assert `FailedPrecondition` and the message.
4. The existing `h3_streaming_tests` stay green; they are the #253 evidence.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_connectrpc
cargo clippy --profile uat -p foundation_connectrpc --no-deps --all-targets -- -D warnings
cargo test --profile uat -p foundation_connectrpc
```

## 8. Done when

- A gRPC client over h3 reads a status for every stream outcome.
- A handler header set before the first message is on the wire on all three transports.
- A header set before the first message reaches the response head on h1, h2 and h3.
- A header set after the first message fails with `FailedPrecondition`.
- `foundation_connectrpc` and every consumer are bumped for the `HandlerExchange` change.
- #253 is closed with the master evidence; #258 is closed by the fix.
