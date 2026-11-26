---
feature: "HTTP and ConnectRPC adapters"
status: "planned"
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

- [ ] **T1 — service.** `DurableService` wraps the store and applies protocol semantics. Test:
      `service_uses_no_transport_type` (compile).
- [ ] **T2 — verbs and statuses.** Create, append, close, read, head and delete. Tests:
      `create_is_idempotent`, `append_after_close_is_rejected`, `close_only_is_idempotent`,
      `head_is_no_store`.
- [ ] **T3 — headers and error mapping.** Full header set and the status table. Tests:
      `gone_is_410`, `fenced_is_403_with_epoch`, `sequence_gap_is_409_with_headers`,
      `closed_append_is_409_with_tail`.
- [ ] **T4 — reads.** Catch-up, empty tail, partial page, long-poll `200`/`204`, `offset=now`.
      Tests: `partial_page_omits_up_to_date`, `closed_tail_returns_immediately`,
      `now_catchup_returns_empty`.
- [x] **T5 — JSON mode.** Validation, flatten, empty array and array reads. Tests:
      `json_array_flattens`, `post_empty_array_is_400`, `put_empty_array_is_valid`.
- [ ] **T6 — SSE.** Event format, control fields, base64, close and reconnect. Tests:
      `sse_control_has_camel_case_fields`, `sse_binary_is_base64`, `sse_closes_after_closed`.
- [ ] **T7 — caching and cursor.** ETag, `304`, `Cache-Control`, cursor interval and jitter.
      Tests: `etag_varies_with_closure`, `if_none_match_is_304`, `cursor_is_monotonic`.
- [ ] **T8 — security.** `nosniff`, CORP, CORS exposure and `429`. Tests:
      `security_headers_on_all_responses`, `cors_exposes_stream_headers`.
- [ ] **T9 — ConnectRPC.** The eight methods and the live stream. Test:
      `rpc_stream_resumes_from_offset`, `http_and_rpc_return_same_chunks`.
- [ ] **T10 — server conformance.** Run the reference suite against `HttpAdapter`. Record every
      failure and skip. Do not close the feature until it passes.
- [ ] **T11 — oracles.** Specification, conformance, golden, failure injection and security
      oracles. Tests: `status_table_matches`, `server_suite_passes`,
      `injected_error_is_typed`, `security_headers_present`.

## 5. Success criteria

- [ ] `cargo test -p foundation_http` and `cargo test -p foundation_connectrpc` green.
- [ ] The HTTP adapter has no valtron dependency.
- [ ] The server conformance suite passes against `HttpAdapter`.
- [ ] Every `StoreError` variant maps to one documented status.
- [ ] `cargo check --workspace --all-targets --profile dev` green.
