---
feature: "HTTP and ConnectRPC adapters"
status: "in progress"
priority: "high"
depends_on: ["02-resumable-reader", "03-idempotent-producer"]
crates: ["foundation_http", "foundation_connectrpc", "foundation_netio"]
---

# Feature 06: HTTP and ConnectRPC adapters

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); Server-Sent Events (SSE);
Content Delivery Network (CDN); Entity Tag (ETag); JavaScript Object Notation (JSON); time to
live (TTL); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

## 1. Goal

Map `DurableService` to HTTP and ConnectRPC with thin adapters. The core stays transport-neutral.
Conformance tests the core and these adapters, with valtron disabled.

## 2. Requirements

1. **Transport-neutral service.** `DurableService<Store>` exposes create, append, read, head,
   close, delete, fork and wait. It has no HTTP type, status code, header, ETag, SSE frame or CDN
   cursor.
2. **HTTP adapter.** `HttpAdapter<S>` owns verbs, headers, statuses, ETag, `Cache-Control`,
   `Stream-Cursor`, SSE, JSON wire, CORS and security headers.
3. **ConnectRPC adapter.** `ConnectRpcAdapter<S>` owns method names, metadata, error codes and
   stream envelopes.
4. **Verbs.** `PUT` create (`201`/`200`), `POST` append/close, `GET` read, `HEAD` metadata,
   `DELETE`, `OPTIONS`. Other methods return `405`.
5. **Headers.** The adapter parses and emits the full request and response header set in
   `requirements.md` section 3.8.
6. **Error precedence.** Append: closed, content type, producer, `Stream-Seq`, JSON. Create:
   TTL/expiry, fork headers, store config. Read: not found, soft-deleted, offset format.
7. **Reads.** Catch-up, long-poll (`200`/`204`) and SSE. The adapter sets `Stream-Up-To-Date`,
   `Stream-Closed` and `Stream-Next-Offset` by the state table.
8. **JSON.** Validate JSON, flatten one array level, reject an empty POST array, accept an empty
   PUT array, return one array on read.
9. **SSE.** `text/event-stream`, `data` and `control` events, camelCase control fields, base64
   for binary streams, final `streamClosed`, no reconnect after close.
10. **Caching and cursor.** ETag with closure suffix, `304`, `Cache-Control`, `Stream-Cursor`
    interval, epoch and jitter.
11. **Security.** `nosniff`, `Cross-Origin-Resource-Policy`, `no-store`, CORS exposure, `429`,
    path validation, TLS.
12. **ConnectRPC.** `Create`, `Append`, `Read`, `Subscribe`, `Head`, `Close`, `Delete` and
    `Fork`. A live read is a server stream. No SSE frames, ETags or CDN cursor.
13. **Server conformance.** The HTTP adapter passes the reference server suite with valtron
    disabled.
14. **Other surfaces.** HTTP is one adapter. WebSocket, DO RPC, Worker and embedded surfaces call
    the same `DurableService`.

## 3. Design

Paths follow the repository layout:

| Item | Path |
|---|---|
| Core service | `foundation_core/src/durable/shared/` |
| HTTP adapter | `foundation_http/src/shared/durable/` with wasm code in `src/wasm/durable/` |
| ConnectRPC adapter | `foundation_connectrpc/src/shared/durable/` with wasm code in `src/wasm/durable/` |

```rust
pub struct DurableService<Store> { store: Store, limits: DurableLimits }

pub struct HttpAdapter<S> { service: DurableService<S> }
pub struct ConnectRpcAdapter<S> { service: DurableService<S> }
```

`DurableLimits` is configuration: page bytes, page chunks, body bytes, long-poll timeout, SSE
recycle interval, cache TTL, cursor interval and cursor epoch. Each has a documented default.

## 4. Tasks

- [x] **T1 — service.** `DurableService` wraps the store and applies protocol semantics. Test:
      `service_uses_no_transport_type` (compile).
- [x] **T2 — verbs and statuses.** Create, append, close, read, head and delete. Tests:
      `create_is_idempotent`, `append_after_close_is_rejected`, `close_only_is_idempotent`,
      `head_is_no_store`.
- [x] **T3 — headers and error mapping.** Full header set and the status table. Tests:
      `gone_is_410`, `fenced_is_403_with_epoch`, `sequence_gap_is_409_with_headers`,
      `closed_append_is_409_with_tail`.
- [x] **T4 — reads.** Catch-up, empty tail, partial page, long-poll `200`/`204`, `offset=now`.
      Tests: `partial_page_omits_up_to_date`, `closed_tail_returns_immediately`,
      `now_catchup_returns_empty`.
- [x] **T5 — JSON mode.** Validation, flatten, empty array and array reads. Tests:
      `json_array_flattens`, `post_empty_array_is_400`, `put_empty_array_is_valid`.
- [x] **T6 — SSE.** Event format, control fields, base64, close and reconnect. Tests:
      `sse_control_has_camel_case_fields`, `sse_binary_is_base64`, `sse_closes_after_closed`.
- [x] **T7 — caching and cursor.** ETag, `304`, `Cache-Control`, cursor interval and jitter.
      Tests: `etag_varies_with_closure`, `if_none_match_is_304`, `cursor_is_monotonic`.
- [x] **T8 — security.** `nosniff`, CORP, CORS exposure and `429`. Implemented by
      `base_headers` and `cors_preflight_headers` in `foundation_http/src/native/durable.rs`
      and `status_for` in `foundation_http/src/shared/durable/status.rs`. Tests:
      `given_a_preflight_when_it_runs_then_it_allows_the_protocol_headers` (CORS exposure) and
      `given_a_rate_limit_error_when_the_headers_are_built_then_retry_after_is_present`.
- [x] **T9 — ConnectRPC.** The eight methods and the live stream. The generated
      `ewe.durable.v1.DurableStreamService` maps create, append, read (one bounded page), head,
      close, delete and fork; `Subscribe` is the server stream. Test:
      `rpc_stream_resumes_from_offset`, `http_and_rpc_return_same_chunks`.
- [x] **T12 — body cap is configuration.** Move the adapter body cap from the constant
      `MAX_BODY_BYTES` to `DurableLimits::max_body_bytes` (default 8 MiB, builder method). A
      declared body over the cap answers `413` and closes. Test:
      `oversized_body_answers_413_from_the_limit`.
- [x] **T10 — server conformance.** Run the reference suite against `DurableHandler`. Record every
      failure and skip. Do not close the feature until it passes.
- [ ] **T11 — oracles.** Specification, conformance, golden, failure injection and security
      oracles. Tests: `status_table_matches`, `server_suite_passes`,
      `injected_error_is_typed`, `security_headers_present`. The status table, the reference
      server suite, the failure-injection oracle and the security oracle pass. Open: no golden
      oracle exists (#1167).

## 5. Success criteria

- [ ] `cargo test -p foundation_http` and `cargo test -p foundation_connectrpc` green.
      Open: the durable HTTP targets pass, including the full `cargo test -p foundation_http
      --features multi` run. `foundation_connectrpc` has one pre-existing failure,
      `conn_io_fallback_polls_instead_of_unsupported` (#1160, medium).
- [x] The HTTP adapter has no valtron stream or task dependency. It uses only the
  injected-clock timer for the live-read timeout.
- [x] The server conformance suite passes against `DurableHandler` (332 pass, 6 skip).
  Command: run `examples/durable_server` with `--features multi`, then run the reference suite.
- [x] Every `StoreError` variant maps to one documented status. Tests:
      `given_a_core_refusal_when_it_maps_then_the_status_matches_the_contract`
      (`tests/durable_status.rs`) and `the_status_table_covers_every_core_refusal`
      (`foundation_connectrpc/tests/durable_stream_service_tests.rs`).
- [ ] `cargo check --workspace --all-targets --profile dev` green.
      Open: no recorded result for the workspace-wide check.

## 6. Open items

1. T11: add the golden oracle (#1167).
2. Run `cargo test -p foundation_connectrpc` after the pre-existing failure #1160 is fixed.
3. Run `cargo check --workspace --all-targets --profile dev`.
