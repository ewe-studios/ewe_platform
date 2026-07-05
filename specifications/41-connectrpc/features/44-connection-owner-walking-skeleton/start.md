---
spec_directory: "specifications/41-connectrpc"
feature: "44-connection-owner-walking-skeleton"
created: 2026-07-05
---

# Start: Connection-owner walking skeleton — end-to-end spine over a real socket

1. Read `plan.md` (esp. the fifth-pass connection-ownership note), then every doc under Normative
   sources in `feature.md` — the decisions are normative; this feature file is the work unit.
2. Read `.agents/skills/rust-clean-code/` and `rust-valtron-usage` (this is pool/reactor work).
3. Land `PushableRequestBody::into_sender` in foundation_netio first (small, verify pushable tests).
4. Build the server `Serve` adapter (connection-owner pump) and the client `open` pump per
   Decision 11 §Connection ownership; replace F22's `feeder`/`collector` at the seam.
5. Prove it with a real loopback-socket test (unary + server-stream). `#[valtron_test]`,
   `--profile uat`; cargo checks with `CARGO_TERM_COLOR=never` + tee.
6. Verify every Acceptance criterion, then update `feature.md` status and F23's dependency note.
