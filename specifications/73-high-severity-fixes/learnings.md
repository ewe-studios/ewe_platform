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

## Feature 05 (2026-11-07)

- A verification feature still needs a sensitivity proof. The fix predates the base, so the
  new test passes there. Break the mechanism on purpose (`TaskStatus::Ignore` in the parking
  arm), see the test fail, then revert. Record the failure message and the time.
- The public valtron pool API cannot make a one-worker pool: it floors the thread count at 3
  and gives at least 2 task workers. Saturate the pool with more queued work instead.
- A feature that changes a default breaks tests that assert the old default. Feature 04
  changed the keep-alive idle timeout to 75 s; `integration_tests.rs` still asserted 120 s.
  Search for the old value in tests in the same change.

## Feature 02 (2026-11-07)

- A client must not compress a frame before the envelope writer. The writer sets
  `FLAG_COMPRESSED` from its own compressor field. A pre-compressed body reaches the peer as
  an uncompressed frame, and the peer decodes garbage. `server_stream_inner` did exactly
  this (found as #378).
- When the client announces compression, the write side must apply it. `client_stream` and
  `bidi_stream` announced `send_compression` in the header but passed `None` to
  `EnvelopeWriter`, so the setting did nothing. One writer configuration fixes all three
  streaming calls.
- Removing a dead parameter from `Client::bidi_stream` is a public API change twice over:
  the generator strings and the macro bodies also carry the parameter list. Search for the
  method name in both crates before the edit.
- The reader tasks are `'static`, so a borrowed config cannot enter them. The registry is a
  cheap `Arc` handle: clone it into the task.
- An integration test cannot construct a private struct. `ClientExchangeConfig` is public,
  so every construction site (including `foundation_proxy`) takes a new required field.
- The announced-encoding bug does not show on every call kind. `client_stream` and `bidi`
  sent unflagged frames and the server still decoded them, because the server trusts the
  per-frame flag. Only `server_stream` failed, because it compressed the payload first. A
  regression test must use the path that actually corrupts data.
- The streaming writer task result is dropped at `spawn_reader_writer`. A typed writer error
  (`ResourceExhausted`) never reaches the caller. Test a writer limit through the writer task
  directly (#380 tracks the missing propagation).

## Feature 09 (2026-11-07)

- A default Cargo feature cannot depend on the target. To keep a dependency out of one target
  while it stays on by default elsewhere, use both an optional feature and a target-scoped
  dependency section, then gate the code with `all(feature = ..., not(target_os = ...))`.
- The absence proof that works without an SDK is `cargo tree --target <target> | grep -c
  <crate>` = 0. A `cargo check` for iOS still needs the iPhoneOS SDK for build scripts
  (`objc2-exception-helper`, `cc-rs`), so on a host with only Command Line Tools the check
  stops before the crate. Do not read that stop as a failure of the gate: confirm the failure
  moved past the gated crate.
- Returning `&dyn Trait` out of a `RwLock<HashMap<.., Box<dyn Trait>>>` is unsound as soon as
  any code can replace the entry. Store `Arc<dyn Trait>` and return the clone. The setter can
  keep its `Box` signature with `Arc::from`.

## Feature 08 (2026-11-07)

- A submodule gitlink can name a commit the remote does not have. Nothing warns until a fresh
  clone runs `git submodule update --init`. The local `.agents` master was two commits ahead
  of the remote, and the superproject pinned the local head. Prove a gitlink is fetchable
  with `git fetch origin <sha>` in a scratch repository, then `git cat-file -t <sha>`.
- Plain `cargo` already isolates worktrees: `target_directory` comes from the workspace root
  of the `Cargo.toml` it runs in. `cargo metadata --manifest-path <other-checkout>/Cargo.toml`
  reads another checkout from inside a worktree without a `cd`.
- `scripts/ci/ci.py` `target_dir()` is at lines 37-50, not 38-47. Line ranges in a design
  drift; cite the function name and check the lines at handoff.

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
