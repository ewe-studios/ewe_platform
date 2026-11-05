---
feature: "foundation_http: every response is written with one shared, offset-tracking, writability-parking writer; idle (not total) send timeout"
status: "done"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_http"]
catalogue_rows: [32, 33]
---

# Feature 61: shared response writer

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problems

1. **Truncated responses (found during feature 41).** `StaticAssetHandler::serve`
   (`src/native/handlers/static_asset.rs`) and `respond::*` (`src/shared/serve/mod.rs`, through
   `render_response`) write the whole response with `write_all`-style code on the non-blocking
   socket. A response larger than the socket send buffer, or a slow client, gets `WouldBlock`
   after a partial write; the connection closes with a truncated response. `respond::*` is used
   by most handlers, so this affects most of `foundation_http`.
2. **Wrong timeout meaning (review of feature 41).** Feature 41's `StaticFileConfig::write_timeout`
   is one deadline for the **whole** response. nginx `send_timeout` (the cited source) is the time
   between two successive writes. With a total deadline, any download that lasts longer than 60 s
   (a large file, a slow link) is cut. The deadline must restart each time the socket accepts
   bytes.

## Design

- Move feature 41's `SendFile::write_all` / `flush` and `WriteWaiter` into one shared module
  (for example `src/native/serve/writer.rs`, `ResponseWriter`): writes a byte slice or a chunk
  stream to the connection, tracks the offset, parks on writability (the same three readiness
  paths), never sleeps on the worker.
- **Idle timeout:** rename the setting to `send_timeout` (keep a deprecated alias if
  `write_timeout` is already public and used) and give it nginx semantics: the timer restarts
  after every write that takes at least one byte. Default 60 s (nginx `send_timeout`). The config
  type it lives on is shared by every user of the writer (static files, static assets,
  `respond::*`); pick the server-level config if one exists, and say where.
- Register the reactor watch (`FdReadiness::register`) lazily, on the first `WouldBlock`, not
  for every response (most small responses never block). Check that the registration is
  removed when the writer drops.
- `StaticFileHandler` uses the shared writer (behaviour unchanged except the timeout meaning).
- `StaticAssetHandler` and `respond::*`: return `ConnectionResult::Async` and write with the
  shared writer. Bodies that are streams stay streams (rule 4); bodies already in memory are
  written from that slice without another copy.
- Search for other writers with the same pattern in the crate
  (`rg -n 'write_all\(' backends/foundation_http/src`) and move each one onto the shared writer,
  or explain in the PR why it cannot block (for example, it writes to a blocking test socket).

## Tests (in `backends/foundation_http/tests/`)

- `respond_large_body_is_byte_exact` (fails on master): a handler returns a 4 MiB body through
  `respond::*`; a slow client; SHA-256 matches and the length is exact.
- `static_asset_large_body_is_byte_exact` (fails on master): same for `StaticAssetHandler`.
- `send_timeout_is_idle_not_total` (fails on the feature 41 code): `send_timeout = 300 ms`; a
  client that reads 16 KiB every 100 ms for 3 s receives the whole file.
- `send_timeout_closes_stalled_client`: a client that stops reading is closed after about
  `send_timeout`.
- The feature 41 tests still pass.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_http --profile dev
rg -n 'thread::sleep' backends/foundation_http/src   # each hit has a comment that says why it is a real boundary
```
Flake check: 20 runs of each new test.

## Done when

- [ ] One shared writer; static file, static asset and `respond::*` use it; no truncation.
- [ ] Idle send timeout; new tests fail on the old code and pass now. §3 backlog entry deleted.
