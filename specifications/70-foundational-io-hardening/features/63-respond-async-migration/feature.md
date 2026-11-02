---
feature: "foundation_http: every native caller of the sync respond::* moves to the async send_* writer; the sync path cannot truncate"
status: "pending"
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

- [ ] No native caller can truncate a response. §3 backlog entry deleted. CI green.
