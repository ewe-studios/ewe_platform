---
feature: "foundation_http: every native caller of the sync respond::* moves to the async send_* writer; the sync path cannot truncate"
status: "done"
lane: "after both lanes merge (touches lane A and lane B crates)"
model: "sonnet"
branch: "fix/io-63-respond-async"
crates: ["foundation_http", "foundation_proxy", "bin/platform templates", "examples/web/wello"]
catalogue_rows: [35, 36]
---

# Feature 63: retire the truncating sync response path

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Starts after the lane A and lane B PRs merge (it edits crates of both lanes). Branch from
`master` at that time.

## Problem (from feature 61)

Feature 61 added the async `native::writer::send_*` helpers and moved `StaticFileHandler`,
`StaticAssetHandler` and `HealthHandler` to them. The sync `respond::*` functions (generic over
`&mut impl Write`) still truncate a response after a partial write on a non-blocking socket.
Remaining callers (re-check with `rg -n 'respond::' --glob '*.rs' backends examples bin crates`):

- `foundation_proxy` (`src/handler.rs`, tests `h2_integration_tests.rs`,
  `h3_integration_tests.rs`, `acme_provisioning_tests.rs`);
- `bin/platform/templates/{web-app,static-html-app}`, `examples/web/wello`;
- in-crate small writes: closing/rejection responses in `native/server/{connection,protocol_detect,mod}.rs`,
  upgrade and SSE heads in `native/upgrade/mod.rs`.

Also from feature 61's review: `ResponseWriter::flush` does not restart the idle timer when the
stream's own buffer drains part of its data, so a large buffered flush can time out while
making progress.

Also from feature 65: a `ResponseWriter`'s idle timer starts when the writer is created, so a
long-idle stream (a gRPC stream with no message for longer than `send_timeout`) times out at
its next blocked write even though the client is reading. Start the idle timer when a write
begins to block (or restart it at the start of each `write_all`), not at creation. Test:
`idle_stream_then_blocked_write_does_not_time_out`.

## Design

1. Move every native caller to `send_*` (returning `ConnectionResult::Async`). The templates
   generate projects; update the template source and the template build test.
2. In-crate small writes: write them through `ResponseWriter` too (they are on the same
   non-blocking socket; "a few hundred bytes into an empty buffer" is likely but not
   guaranteed after a pipelined response).
3. Then make the sync `respond::*` unusable on a non-blocking native socket: remove it from the
   native public API, or keep it only for the cases that need a sync `Write` (wasm, tests with a
   `Vec<u8>`), renamed so the name says it is for in-memory writers (for example
   `respond_to_buffer::*`). Say which.
4. `flush` progress: if the stream exposes how much its buffer holds, restart the idle timer when
   it shrinks; otherwise flush the internal buffer through `write_all` so progress is seen.

## Tests

- `proxy_large_response_is_byte_exact` (fails before step 1): the proxy test server returns 4 MiB.
- Template build test: generated projects build without warnings and use `send_*`.
- `buffered_flush_restarts_idle_timer` (fails on the feature 61 code if the buffer can hold more
  than one socket write).

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_http --profile dev
python3 scripts/ci/ci.py run foundation_proxy --profile dev
cargo check --workspace --all-targets --profile dev
rg -n 'respond::' --glob '*.rs' backends examples bin crates   # only buffer/wasm uses, if kept
```

## Done when

- [x] No native caller can truncate a response. §3 backlog entry deleted. CI green.

## Result

Choice for design step 3: the synchronous helpers were **renamed**, not removed:
`shared::serve::respond` is now `shared::serve::respond_to_buffer`. They remain
for in-memory writers only (tests over a `Vec<u8>`, wasm). Every native caller
moved to the async writer:

| Caller | Was | Now |
| --- | --- | --- |
| `foundation_http` `native/server/connection.rs` (505, 400, 408, 404, 502, `100 Continue`) | `respond::*` | `send_text` / `send_not_found` / `send_continue_100`, driven by `HandlerState::Awaiting` and a new `ContinueWrite` state |
| `native/server/protocol_detect.rs` (505, 503 refusals) | `respond::text` | `send_text` driven by a `FutureTask` (the drain guard rides in it) |
| `native/server/mod.rs` (accept-loop 503) | `respond::text` | removed: the branch runs only when the executor is unavailable, so no task can drive an async response. It logs and closes instead of writing a half response. |
| `native/upgrade/mod.rs` (WebSocket 101, SSE head) | `http_render_to_writer` | `accept_websocket` and `SseStream::new` are now `async` and use `ResponseWriter` |
| `shared/handlers/health.rs` (`ServeWriter`, in-memory/wasm) | `respond::json` | `respond_to_buffer::json` |
| `foundation_proxy` tests `h2_*`, `h3_*`, `acme_*` | `respond::*` | `send_text` / `send_json` / `send_not_found` / `ResponseWriter` (ACME handler is async) |
| `foundation_http` server tests | `respond::*` on real sockets | `send_text` / `send_json` |
| templates `web-app`, `static-html-app`, `examples/web/wello` | `respond::html` / `not_found` | `send_html` / `send_not_found` |

New writer API: `send_continue_100` and `render_message`; `RawStream::write_buffer_len`
in `foundation_netio` exposes how much the stream's write buffer still holds.

Timer fixes:

1. `ResponseWriter::write_all` and `flush` restart the idle timer at their start.
   The timer now measures a stall inside one operation, not the writer's age.
2. `flush` compares the stream's buffered length before and after each attempt
   and restarts the timer whenever the buffer shrank, so a flush that keeps
   draining a slow client is never cut off.

Version bumps: `foundation_netio` 0.0.4 → 0.0.5 (new public method, every
workspace requirement moved), `foundation_http` 0.0.3 → 0.0.4 (public writer and
upgrade API changes, every requirement and both template pins moved).

`ConnectionHandler::handle_awaiting` now polls the handler future before it
applies the shutdown check. A handler that is only writing its answer completes
during the drain (the graceful-drain test), while a future that is still
`Pending` is dropped at the shutdown checkpoint (the feature-08 test). The
crate's clippy debt is cleared for the default, `multi`, `multi quic` and
`multi quic ssl` trees; `ci/packages.toml` now denies clippy warnings for
`foundation_http`.

