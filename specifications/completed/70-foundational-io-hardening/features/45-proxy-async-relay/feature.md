---
feature: "foundation_proxy: forward_http relays as ConnectionResult::Async; cross-process state safety; cloudflare feature dependency"
status: "done"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_proxy"]
catalogue_rows: [7, 8, 9]
---

# Feature 45: proxy relay, state lock, unused feature dependency

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## E1. `forward_http` relays on a throwaway thread (backlog §9)

`src/handler.rs` (~line 123) spawns a `std::thread` per proxied connection and returns
`ConnectionResult::Take`; `src/forward.rs:233` `forward_http` does the relay.

- Make the relay an `async fn` whose future is `'static + Send` (owned connection halves,
  owned config `Arc`), and return `ConnectionResult::Async`. Upstream reads and writes await
  readiness (the netio async client / connection readiness used elsewhere after spec 70).
- Keep streaming in both directions (rule 4): no full-body buffering.
- Test: `proxy_relay_does_not_spawn_threads` (count threads before and after 50 proxied
  requests with a 1-worker pool; no growth), `proxy_relay_holds_no_worker` (a slow upstream
  does not block a second request on a 1-worker pool), and the existing proxy tests.
- Also list in the PR body the other `std::thread::spawn` sites (`passthrough.rs` ~177,
  ~221, ~279; `server.rs` ~204, ~235, ~352). For each: keep with a comment that says why it is
  a real boundary (an accept loop thread is acceptable), or record a backlog entry.

## E2. State write lock is per process (review finding)

`src/persistence.rs` serialises read-modify-write with an in-process `AsyncMutex`. Two proxy
processes on one store still race. Decide and implement one:

- (preferred) make the persisted write conditional (compare a version/etag column, or use the
  store's atomic update) and retry on conflict, with attempts on config; or
- document that one store serves one proxy process, enforce it with a lock row/lease
  acquired at start, and fail fast if another process holds it.

Test: `two_stores_on_one_backend_do_not_lose_updates` (two `Persistence` instances on one
file/SQLite backend, 100 interleaved updates each, final state has all 200).

## E3. `cloudflare` feature has an unused dependency (backlog §9)

`foundation_deployment_cloudflare` is named only in doc comments. Drive the ACME DNS-01
Cloudflare path through it if that path exists and is half-built; otherwise remove the
dependency from the feature (keep the feature name if other code checks it). Say which.
Verify: `cargo check -p foundation_proxy --features cloudflare 2>&1 | rg unused` is empty.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_proxy --profile dev
```
Flake check: 20 runs of each new test.

## Done when

- [x] E1–E3 done; the two §9 entries deleted; new entries for any thread site kept as a
      problem. CI green.
