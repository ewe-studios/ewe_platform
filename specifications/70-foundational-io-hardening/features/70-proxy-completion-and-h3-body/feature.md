---
feature: "foundation_proxy: no polling splice, non-blocking io_uring upstream dial, streamed H3 forward body"
status: "pending"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_proxy"]
catalogue_rows: [60, 61, 62]
---

# Feature 70: proxy leftovers from feature 65

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 65 (backlog §9).

## E1. `splice_bidirectional` is a polling splice used only by a test

`src/passthrough.rs` ~241: a blocking copy for generic `Read + Write` pairs that sleeps 1 ms
when idle; only `completion_splice_tests` calls it. Move that test onto `aio::splice` (the async
splice from feature 65, with the timer fallback for completion sockets) and delete
`splice_bidirectional`. If the test cannot use the async splice, say why and stop.

## E2. The io_uring upstream dial (`connect_completion`) is a blocking connect

Make it non-blocking: submit the connect through the completion layer and park until it
completes (the same way feature 65's `dial_tcp` parks on writability for epoll sockets), with
the existing connect timeout from config. Test: `completion_dial_does_not_hold_worker`
(1-worker pool, dial a blackholed address with a 2 s timeout; a second request is answered
meanwhile). If the completion layer has no async connect, add it in the lane A crate that owns
it (foundation_netio or foundation_nativeapis) or stop and report.

## E3. The H3 forward collects the whole response body (rule 4)

`src/h3_proxy.rs`: stream the upstream body to the H3 response chunk by chunk (send each chunk
with the H3 stream's flow control; no full-body buffer). Test:
`h3_forward_streams_large_body` (16 MiB upstream body; peak buffered bytes under 1 MiB,
measured with a counting body source; SHA-256 matches).

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_proxy --profile dev
cargo test -p foundation_proxy --profile dev --features quic
```
Flake check: 20 runs of each new test.

## Done when

- [ ] Three §9 entries fixed and deleted. CI green.
