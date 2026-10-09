---
spec_directory: "specifications/41-connectrpc"
feature: "22-router-dispatch"
created: 2026-07-03
---

# Start: Router, ProcedureMeta, erased handlers, ConnectRpcHandler (D08)

1. Read `plan.md`, then every doc under Normative sources in `feature.md` — the decisions
   are normative; this feature file is the work unit, never a substitute.
2. Read `.agents/skills/rust-clean-code/` and (for pool/async work) `rust-valtron-usage`.
3. Implement the Scope bullets 100% (no partial scaffolding — repo rule); tests in `tests/`.
4. Use `#[valtron_test]` for pool tests; cargo checks with CARGO_TERM_COLOR=never + tee.
5. Verify every Acceptance criterion, then update `feature.md` status.
