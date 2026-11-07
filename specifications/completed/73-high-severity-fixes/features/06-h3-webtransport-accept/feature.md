---
feature: "Verify the H3 WebTransport accept keeps a pending request, then close #273"
status: "completed"
priority: "high"
depends_on: []
crates: ["foundation_netio"]
issues: [273]
model: "haiku"
branch: "spec73/f06-h3-webtransport-accept"
---

# Feature 06: HTTP/3 WebTransport accept — verification

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** HTTP/3 (h3); WebTransport (WT); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Close [#273](https://github.com/ewe-studios/ewe_platform/issues/273) with evidence. The
defect is already fixed on master.

## 2. Measured state (2026-11-07)

`backends/foundation_netio/src/http3/connection.rs:441` `poll_accept_webtransport` keeps a
`pending_accept: Option<PendingAccept<C::BidiStream>>` field (line 180). The field doc says
"kept across polls so it is never lost".

- A pending headers read returns `Stream::Pending(())` and keeps the stream (line 487).
- A non-WebTransport request is returned as `WtAccept::Request` with its headers
  (lines 473-480).
- A write that is not ready keeps the stream (line 498).

The issue's two reported failures cannot happen in this code. This is a verification
feature, not an implementation feature.

## 3. Tasks

- [ ] Run `rg -n "pending_accept" backends/foundation_netio/src/http3/connection.rs`.
      Confirm the three cases above.
- [ ] Confirm a test covers a request that arrives after one pending poll. If none exists,
      add `a_request_after_a_pending_poll_is_accepted` in
      `backends/foundation_netio/tests/` (registered in its test module) with a mock
      `QuicConnection` that returns Pending once, then a stream.
- [ ] Run the verify commands.
- [ ] Close #273 in the PR body with the code lines and the test name.

## 4. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_netio
cargo clippy --profile uat -p foundation_netio --no-deps --all-targets -- -D warnings
cargo test --profile uat -p foundation_netio
```

## 5. Done when

- The three cases are confirmed against the code, with line numbers in the PR.
- A test pins the pending-poll case.
- #273 is closed with the evidence.

## 6. Result (2026-11-07)

Branch `spec73/f06-h3-webtransport-accept`, commit `30dfe9bad`, merged into
`spec73/high-severity-fixes` as `1834f0fcb`. The commit adds three doc lines only. #273 is
closed.

1. **Measured path.** The field `pending_accept` is at `http3/connection.rs:180`. Line 444
   reuses it with `as_mut()` and line 447 inserts a new stream.
   - Case 1: the pending HEADERS arm at line 487 returns `Stream::Pending(())` and keeps the
     stream.
   - Case 2: lines 473-480 return `WtAccept::Request { request, headers }` for an ordinary
     request.
   - Case 3: the pending write arm at line 498 keeps the partial `Bytes` in
     `pending.response`.
2. **Pinning test.** No duplicate was added. The existing test
   `a_stream_accepted_before_its_headers_is_not_lost` at
   `tests/http3/webtransport_accept_tests.rs:98` sends a reserved frame before the HEADERS,
   polls while the HEADERS are pending, then asserts the accepted request with its `:path`.
   `an_ordinary_request_is_handed_back_with_its_headers` at line 145 pins case 2. The commit
   names `pending_accept` and #273 in the first test's doc comment.
3. **Old code.** The fix predates the spec (`ed492ed3c`, 2026-10-27). The pre-fix API
   returned `Result<bool, H3Error>` and dropped the accepted stream; the test uses the
   `WtAccept` API and cannot compile against it. The drop is visible in
   `git show ed492ed3c -- backends/foundation_netio/src/http3/connection.rs`.
4. **Counts.** `foundation_netio` default: 1302 passed. With `--features quic --test http3`:
   59 passed.
5. **Deviation.** The literal verify command has no `--features quic`, so Cargo skips the
   `http3` target (`required-features = ["quic"]`). The target was run separately. Filed #387.
6. **New defects.** Filed #386 (medium: the quic-gated code fails clippy with `-D warnings`,
   74 diagnostics), #387 (medium: the default runs skip every quic, http3 and webtransport
   target) and #388 (low: the http3 and webtransport suites run twice).
