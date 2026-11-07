---
feature: "Verify the H3 WebTransport accept keeps a pending request, then close #273"
status: "pending"
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
