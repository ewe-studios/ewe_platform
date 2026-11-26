---
feature: "Durable Streams client"
status: "planned"
priority: "high"
depends_on: ["01-durable-stream-types"]
crates: ["foundation_netio", "foundation_core"]
---

# Feature 07: Durable Streams client

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); Server-Sent Events (SSE);
JavaScript Object Notation (JSON); time to live (TTL); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Add the Rust client core in `foundation_core::durable::client`. It is plain async. It defines
the transport trait it expects, and a host implements it above the core. Then add the required valtron client wrapper. The client core must pass the
reference client conformance suite with its adapter.

## 2. Requirements

1. **Shared types.** The client uses `foundation_core::durable::shared` types. It does not define a
   second offset, cursor or error model.
2. **Driver.** `ClientDriver` provides the transport, clock and retry policy. The client business
   API takes a driver. Embedded, HTTP, WebSocket and DO are client drivers. The transport is
   injected. It uses `foundation_netio`. No Tokio.
3. **Client handle.** `DurableClient` has a base URL, default headers, a dynamic header provider,
   a retry policy and a clock.
4. **Stream handle.** `StreamHandle` has create, connect, append, head, delete, close, read and
   producer.
5. **Read builder.** It supports offset, live mode, timeout, `max_chunks`, cursor and headers.
   `LiveMode` is `Off`, `Auto`, `LongPoll` and `Sse`.
6. **Chunk.** A chunk carries data, next offset, `up_to_date`, `closed`, cursor and status.
7. **Read modes.** Catch-up, long-poll, SSE and auto. Auto catches up, then prefers SSE, then
   falls back to long-poll.
8. **Offsets.** `-1`, `now`, stored offsets and an omitted offset. An omitted live read starts at
   the beginning.
9. **JSON.** Parse arrays. Surface `PARSE_ERROR` on bad JSON. Never turn a parse error into an
   empty success.
10. **SSE.** Parse `data` and `control` events. Ignore unknown events. Strip one leading space.
    Preserve U+0085, U+2028 and U+2029. Decode base64 when the header is present.
11. **Retry.** Retry `500`, `502`, `503`, `504`, `429` and network errors with backoff. Honor
    `Retry-After`. Never retry `400` or `404`. Stop after a bound.
12. **Append safety.** A plain append does not retry. A producer append retries.
13. **Errors.** Map to the conformance codes: `NETWORK_ERROR`, `TIMEOUT`, `CONFLICT`,
    `NOT_FOUND`, `SEQUENCE_CONFLICT`, `STREAM_CLOSED`, `INVALID_OFFSET`, `UNEXPECTED_STATUS`,
    `PARSE_ERROR`, `INTERNAL_ERROR`, `NOT_SUPPORTED`, `INVALID_ARGUMENT`. A `404` message
    includes the path.
14. **Producer.** Support batching, `max_batch_bytes`, `max_batch_items`, `max_in_flight`,
    linger, epoch, auto-claim, close and detach. Key state by `(path, producer_id, epoch)`.
15. **Dynamic headers.** Evaluate header and query functions per request.
16. **Validation.** Reject a negative retry or epoch, a zero delay, a zero batch size and an
    empty producer ID. Put the field name in the message.
17. **Adapter.** A separate binary speaks JSON lines. It announces accurate features. It
    normalizes append, delete and close to `200`. It reports duplicates as `204` with
    `duplicate: true`.
18. **Valtron wrapper.** A wrapper turns the client reader into a `StreamIterator` with
    `Stream::Wait` and `WakeableStream`. It must not change client outcomes.
19. **Client conformance.** The core and adapter pass the reference client suite with valtron
    disabled.
20. **BDD scenarios.** Client behaviours are Given/When/Then scenarios. The same scenario runs
    on every client driver.
21. **wasm32.** The client core builds and runs on wasm32. The transport and clock are drivers.

## 3. Design

Paths follow the repository layout:

| Item | Path |
|---|---|
| Client core and transport trait | `foundation_core/src/durable/shared/client/` |
| Native transport | `foundation_netio/src/native/durable.rs` |
| wasm transport | `foundation_netio/src/wasm/durable/` |
| Valtron client wrapper | `foundation_core/src/durable/shared/client/valtron.rs` |

```rust
pub struct DurableClient { /* transport, base URL, headers, retry, clock */ }

impl DurableClient {
    pub fn stream(&self, name: &StreamName) -> StreamHandle;
    pub fn connect(&self, name: &StreamName) -> Result<StreamHandle, StreamError>;
}

impl StreamHandle {
    pub async fn create(&self, spec: CreateSpec) -> Result<CreateOutcome, StreamError>;
    pub async fn append(&self, req: AppendRequest) -> Result<AppendOutcome, StreamError>;
    pub async fn head(&self) -> Result<StreamHead, StreamError>;
    pub async fn delete(&self) -> Result<DeleteOutcome, StreamError>;
    pub async fn close(&self, req: CloseRequest) -> Result<AppendOutcome, StreamError>;
    pub fn read(&self) -> ReadBuilder;
    pub fn producer(&self, id: ProducerId) -> ProducerBuilder;
}
```

## 4. Tasks

- [x] **T1 — client core.** Types, transport and the six verbs. Tests:
      `create_is_idempotent`, `append_returns_offset`, `head_returns_metadata`.
- [x] **T2 — catch-up and long-poll.** Offset handling, byte-exact resumption and `204`.
      Tests: `read_resumes_byte_exactly`, `long_poll_timeout_is_up_to_date`.
- [ ] **T3 — SSE and auto.** Parser, base64, control events, unknown events, auto fallback.
      Tests: `sse_decodes_base64`, `sse_ignores_unknown_event`, `auto_falls_back_to_long_poll`.
- [ ] **T4 — JSON and errors.** Parse arrays, `PARSE_ERROR`, error codes and path context.
      Tests: `json_parse_error_is_typed`, `not_found_message_has_path`.
- [ ] **T5 — retry.** `500`, `503`, `429`, exhaustion and no-retry rules. Tests:
      `retries_transient_errors`, `does_not_retry_404`, `retry_exhaustion_is_an_error`.
- [ ] **T6 — producer.** Batching, epoch, sequence, auto-claim, dedup, close and detach. Tests:
      `producer_deduplicates_retry`, `auto_claim_moves_epoch`, `detach_does_not_close`.
- [x] **T7 — dynamic headers and validation.** Per-request headers and config validation. Tests:
      `dynamic_header_counter`, `invalid_epoch_is_rejected`.
- [x] **T8 — adapter.** JSON-line commands and results. Normalize success statuses. Test:
      `adapter_reports_200_for_append`.
- [x] **T9 — valtron wrapper.** `StreamIterator` and `WakeableStream` over the client reader.
      Test: `client_reader_parks_and_wakes`.
- [x] **T10 — client conformance.** Run the reference client suite with the adapter. Record every
      failure and skip. Do not close the feature until it passes.
- [ ] **T11 — wasm32.** Build the client core for wasm32 and run the BDD scenarios with a wasm
      driver. Test: `wasm_client_roundtrip`.
- [ ] **T12 — oracles.** Differential, conformance, property and failure injection oracles.
      Tests: `client_matches_reference`, `client_suite_passes`, `retry_is_bounded`.

## 5. Success criteria

- [ ] `cargo test -p foundation_netio` green; client tests live in `tests/durable/`.
- [ ] The client core has no valtron dependency.
- [ ] The client conformance suite passes with the adapter.
- [ ] The valtron wrapper tests pass with valtron enabled.
