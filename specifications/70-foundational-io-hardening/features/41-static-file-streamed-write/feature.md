---
feature: "foundation_http static file handler: async, streamed from disk, no duplicate bytes, no sleeps"
status: "pending"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_http"]
catalogue_rows: [1]
---

# Feature 41: Static file handler writes correctly and does not hold a worker

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (verified on master)

`backends/foundation_http/src/native/handlers/static_file.rs` (`StaticFileHandler::serve`):

1. Reads the whole file into memory (`SendSafeBody::Bytes(contents)`) and renders the whole
   response into one `Vec` (`wire`). Rule 4 breach.
2. Writes with `conn.write_all(&wire)` in a retry loop on `WouldBlock`. The socket is
   non-blocking. `write_all` can write part of `wire` and then return `WouldBlock`; the
   retry calls `write_all(&wire)` again **from byte 0**, so the client gets duplicated bytes.
   Any file larger than the socket send buffer (often 64–200 KiB) can be corrupted.
3. Sleeps on the worker thread (`thread::sleep(50 ms × attempt)`), up to 2.75 s, and
   retries also on `ConnectionReset` / `BrokenPipe` / `ConnectionAborted` (a dead peer
   never recovers). `MAX_RETRIES = 10` is a constant (rule 8).

## Design

Return `ConnectionResult::Async(ServeFuture)` (added by spec 70 feature 08; read
`foundation_http/src/shared/serve/mod.rs` and an existing `Async` user first).

- Open the file, read its metadata (length, modified time) and write the head with
  `Content-Length`. Keep the existing content type, `ETag`/`Last-Modified` and `HEAD`
  behaviour if present.
- Stream the body: read the file in chunks of `StaticFileConfig::chunk_size` (new field, with a
  builder method; default 64 KiB, the nginx `output_buffers` order; say so in the doc comment)
  and write each chunk **tracking the offset**: write the unwritten tail only; on
  `WouldBlock` await connection writability (the readiness the async serve path already
  uses), never sleep.
- Errors: `ConnectionReset`, `BrokenPipe`, `ConnectionAborted` close at once (log at debug).
  A total write deadline `StaticFileConfig::write_timeout` (default 60 s, nginx
  `send_timeout`) closes a stalled client.
- File reads: on Linux use the crate's existing offload/uring path if one exists for file I/O
  (search `foundation_nativeapis`/`foundation_core` for the offload pool used by spec 70
  feature 20 E4); otherwise read inline in chunks (a 64 KiB page-cache read is fast) and write
  the reason in a comment. Do not add a new thread pool.
- Remove `MAX_RETRIES` and every `thread::sleep` from the file.

## Tests (in `backends/foundation_http/tests/`, must fail on master where marked)

- `static_file_large_body_is_byte_exact` (fails on master): serve an 8 MiB file of a
  known pattern; the client reads slowly (sleep 1 ms per 16 KiB read) so the server meets
  `WouldBlock`; assert SHA-256 of the received body equals the file's.
- `static_file_does_not_hold_worker`: one worker; while a slow client downloads a large
  file, a second request to another route answers within 500 ms.
- `static_file_peer_reset_closes_without_retry`: client closes after the head; the handler
  ends within 200 ms (no 2.75 s of sleeps).
- `static_file_limits_come_from_config`: chunk size and write timeout are read from config.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_http --profile dev
rg -n 'thread::sleep|MAX_RETRIES' backends/foundation_http/src/native/handlers/static_file.rs   # no output
```
Flake check: 20 runs of each new test.

## Done when

- [ ] T0 line in PR body. Byte-exact test fails on master and passes now (show both).
- [ ] No sleep, no constant limit, no whole-file buffer in the handler.
- [ ] Backlog: no entry needed after the fix (it was never in the backlog); write the fix in
      progress.md.
