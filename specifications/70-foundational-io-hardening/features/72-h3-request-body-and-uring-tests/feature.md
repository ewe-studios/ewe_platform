---
feature: "foundation_proxy H3: stream the request body and park on the waker; io_uring tests fail loudly when io_uring is missing in CI"
status: "pending"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_proxy", "foundation_iogate"]
catalogue_rows: [68, 69]
---

# Feature 72: H3 request body; io_uring test skips

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 70 (backlog §9).

## E1. The H3 proxy buffers the whole request body and polls on a 1 ms timer

`src/h3_proxy.rs` `read_body` collects the whole request body (rule 4), and `read_headers` /
`read_body` poll with a 1 ms timer instead of parking on the connection waker. Stream the
request body to the upstream as it arrives (the same pump pattern feature 65 E5 uses for H1:
a bounded pushable pipe, `ProxyConfig::max_request_body` → 413), and park on the waker
(`park_op`, as feature 70's response path does). Tests: `h3_request_body_is_streamed`
(16 MiB upload; peak buffered under 1 MiB via the `BufferedBytesGauge`),
`h3_idle_request_uses_no_cpu` (thread CPU under 20 ms while a client holds a request open
for 1 s without sending).

## E2. io_uring tests skip silently when the kernel has no io_uring

They print a skip and pass, so CI can lose this coverage without anyone noticing. Keep the
skip for local machines, but fail when an environment variable says io_uring is expected
(for example `EWE_REQUIRE_IO_URING=1`), and set that variable in the Linux CI job for the
crates that have io_uring tests (check the CI image's kernel supports it first; if it does not,
record that in the backlog instead of setting the variable).

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -p foundation_proxy --profile dev --features quic
python3 scripts/ci/ci.py run foundation_proxy --profile dev
```
Flake check: 20 runs of each new test.

## Done when

- [ ] H3 request body streamed and capped; no 1 ms polling; io_uring tests cannot skip in CI
      unnoticed. §9 entries deleted. CI green.
