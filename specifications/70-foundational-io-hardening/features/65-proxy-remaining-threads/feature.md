---
feature: "foundation_proxy: no thread per connection and no sleep-poll loops (upgrade, TCP tunnel, gRPC-Web/Connect relays, TCP passthrough, control socket); locked state reads"
status: "pending"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_proxy"]
catalogue_rows: [43, 44, 45, 46, 47, 48]
---

# Feature 65: the rest of the proxy's threads and polling loops

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 45 (all in `docs/bug-backlog.md` §9). Feature 45 made the ordinary HTTP relay
async; these paths still use a thread per connection or sleep-poll.

## E1. Relays that still run on a throwaway thread (`handler.rs`, `relay_blocking`)

Protocol upgrade (WebSocket), raw TCP tunnel, gRPC-Web and Connect translators to a `grpc://`
backend. Make each an async splice in a `ConnectionResult::Async` future: two directions, each
reads with read readiness and writes with `ResponseWriter`-style write parking (feature 61), both
directions end cleanly when either side closes (half-close where the protocol allows). The
gRPC translators need an async HTTP/2 client call; use the netio async client used by
`forward_http` after feature 45. If one path needs an API that does not exist in a lane A crate,
stop and report.

## E2. `TcpPassthrough` thread per client and poll sleeps (`passthrough.rs` ~221, and the 5 ms / 20 ms sleeps)

Accept loop: park on listener readiness (`FdReadiness`), as `foundation_http`'s accept loop does
after spec 70 feature 05/31. Per client: an async splice task on the valtron pool (same splice
as E1). UDP relay: park on socket readiness instead of a poll sleep.

## E3. Thread sleeps in production polling loops

`forward.rs` `write_head_blocking`, `h2_proxy.rs`, `h3_proxy.rs`, `health.rs`, `control.rs`,
`server.rs` `drain_complete`. For each: replace the sleep with a readiness or timer park
(valtron timer), or, where it is a real blocking boundary on a dedicated thread, keep it with a
comment that says why and make the interval a config field (rule 8). Table in the PR.

## E4. The control socket starts a thread per connection (`control.rs` ~102)

Serve control connections as valtron tasks (async read/write on the Unix socket).

## E5. Upstream request body is pulled by the upstream client task on a pool worker

Measure first: does a slow client upload hold a pool worker while `forward_http` runs? Test
`slow_client_upload_does_not_hold_worker` (2-worker pool, client sends the body 1 byte/50 ms for
2 s, a second request answers within 500 ms). If it fails, make the request body an async
stream from the client connection into the upstream request (read readiness), capped by
`ProxyConfig::max_request_body` (new, default 1 MiB nginx `client_max_body_size`, 0 = unlimited
for streaming uploads — document).

## E6. The proxy's plain state `load` is not under the cross-process lock

Feature 45 locks load-change-save; a plain `load` can read a half-written file (the store writes
in place; feature 56 E7 makes the store write atomically). Take the shared (`LOCK_SH`) flock for
`load`. Test: `load_never_sees_a_torn_file` (one process-like writer saving large states in a
loop, a reader loading 1000 times, every load parses).

## Tests

Each E names its test. Plus `proxy_upgrade_does_not_spawn_threads` and
`tcp_passthrough_does_not_spawn_threads` (thread count from `/proc/self/task`, as feature 45's
test). Each must fail on the current code where it measures a current bug.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_proxy --profile dev
rg -n 'thread::sleep|thread::spawn' backends/foundation_proxy/src   # each hit has a comment naming a real boundary
```
Flake check: 20 runs of each new test.

## Done when

- [ ] No thread per connection; no sleep-poll on a worker; remaining dedicated threads justified.
- [ ] The six §9 entries above deleted. CI green.
