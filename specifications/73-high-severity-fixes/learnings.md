# Learnings — high-severity defect fixes

_Seeded at spec creation. Updated after each feature PR. See
`.agents/skills/specifications-management/skill.md`._

## Design pass (2026-11-07)

- A high issue can be stale. #253 says `send_h3_response` drops the trailers, but on master
  the function writes a trailer frame. #258 names `HandlerConnEnds.response_headers`, which
  no longer exists. Each feature must re-measure the defect on master before an agent plans
  code. Do not implement from the issue text.
- A crate version bump on a 0.0.x version is a breaking change for every requirement. The
  caret rule `^0.0.9` accepts only `0.0.9`. The spec 72 feature 08 agent updated seven
  requirement lines and two example lockfiles, and missed two more example lockfiles. Sweep
  every `Cargo.lock` in the repository after a bump:
  `rg -l 'name = "<crate>"' --glob '**/Cargo.lock'`.
- #265 does not change behaviour. The existing clamp already maps every wire value. The
  feature proves the change with a source gate, not with a failing test. Say that in the
  feature file; a reviewer must not expect a red test.
- wasmtime cannot run on iOS. Cranelift is a JIT, iOS forbids W^X and runtime executable
  memory, and wasmtime has no interpreter backend. The fix is a dependency gate. If iOS must
  run Wasm, the only path is an interpreter runtime such as `wasmi`.
- `foundation_netio` owns the WebSocket task. Issue #288 names
  `websocket/native/task.rs` under `foundation_http`; the live file is in
  `foundation_netio`. Read the path from the file system, not from the issue.
- The four high fix branches (#334, #340, #347, #348) are not spec 73 features. They close
  before the spec starts so that the spec's crates start from a clean master.

## Feature 01 (2026-11-07)

- `Code::from_u32` already exists in `shared/error/code.rs` (one match table, 1..=16) and
  returns `Option<Self>`. Use it. Do not add a second table for the same mapping.
- `foundation_connectrpc` has no `tests/mod.rs`. Cargo discovers every top-level file in
  `tests/`. A brief that says "register the test in `tests/mod.rs`" is wrong for this crate.
- An integration test can only call the public API. A `pub(crate)` parser needs a
  `#[doc(hidden)] pub` test hook. This crate already uses that pattern (`h2_pump_poll_count`),
  so the hook is not a new convention and needs no version bump.

## Creation (2026-11-07)

- The 14 high-severity issues were latent inside the retired umbrella backlog. No one
  scheduled them because the checklists did not carry labels. Filing each one separately is
  what made them visible.
- Seven of the 14 are in one crate, `foundation_connectrpc`. They share files, so one agent
  per issue would edit the same files at once. Group them by file and wire path instead.
- Two of the 14 are not code defects: the iOS target needs a dependency gate, and the macOS
  VM needs the iPhoneOS SDK. They belong in the same queue because they block a supported
  platform target.
- #265 is an `unsafe` block with a manual range check. The behaviour does not change when the
  check becomes a safe conversion, so no behavioural test fails on the old code. State the
  source gate in the feature file instead of inventing a test.
