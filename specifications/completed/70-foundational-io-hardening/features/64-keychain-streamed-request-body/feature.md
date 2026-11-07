---
feature: "foundation_keychain: the server reads the request body inside the async future as a bounded stream"
status: "done"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_keychain"]
catalogue_rows: [37]
---

# Feature 64: keychain request body read

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (found in feature 44)

`ServeAdapter::serve` (`src/server/native.rs`) calls `read_body`, which runs
`try_collect_bytes` **before** it returns `ConnectionResult::Async`. So a worker blocks while
the client sends the body, and the whole body is held in memory with no size cap (rules 4 and
8). A slow or large upload holds a worker.

## Design

- Move the body read into the async future: read the request body as an async stream
  (`AsyncSendSafeBody` / the async body API that foundation_http's async handlers use; find
  the one used after spec 70 features 03 and 08).
- Cap it: `max_request_body` on the keychain server config, with a builder method; default
  1 MiB (the nginx `client_max_body_size` default — say so). Over the cap: answer 413 and close,
  without reading the rest.
- The JSON handlers need the whole (small) body to parse: collect up to the cap, then parse
  (allowed by rule 4 for a small body that the next step needs).

## Tests (`backends/foundation_keychain/tests/`)

- `slow_request_body_does_not_hold_worker` (fails on the feature 44 code): a client sends the
  body slowly (1 byte every 50 ms for 2 s); on a 2-worker server a second request answers
  within 500 ms.
- `request_body_over_cap_is_413`: `max_request_body = 1 KiB`, a 2 KiB body → 413.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_keychain --profile dev
```
Flake check: 20 runs of each new test.

## Done when

- [x] Body read inside the future, capped from config; first test fails before, passes after.
- [x] §22 entry deleted. CI green.
