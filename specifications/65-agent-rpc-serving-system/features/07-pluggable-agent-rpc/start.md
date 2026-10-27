---
workspace_name: "ewe_platform"
spec_directory: "specifications/65-agent-rpc-serving-system"
feature_directory: "specifications/65-agent-rpc-serving-system/features/07-pluggable-agent-rpc"
this_file: "specifications/65-agent-rpc-serving-system/features/07-pluggable-agent-rpc/start.md"
created: 2026-10-22
---

# Start: Pluggable Agent RPC

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** remote procedure call (RPC); deterministic simulation testing
(DST); continuous integration (CI); pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## Read first

1. `CLAUDE.md` at the repository root. All rules apply. Rules 8 (limits are configuration),
   10 (keep `progress.md` and `LEARNINGS.md` current), 11 (side effects are injected) and 12
   (write in STE) are central to this feature.
2. [`feature.md`](./feature.md): overview, glossary, architecture, the feature split (section
   10), open questions. Then the `feature.md` of the feature you work on (08 to 14).
3. [`deterministic-simulation-testing.md`](./deterministic-simulation-testing.md): why every
   side effect is a trait.
4. The topic file for your task: [`env.md`](./env.md), [`auth.md`](./auth.md),
   [`registry-and-models.md`](./registry-and-models.md), [`hosts.md`](./hosts.md) or
   [`proto.md`](./proto.md).
5. `../../progress.md` and `../../LEARNINGS.md` for the state of spec 65.

## Workflow

1. Do task T0 first. Merge `master` into `65-f01-agent-rpc` with a merge commit (no rebase,
   no force-push). Adapt the crate to the async storage traits and the async `AgentSession`.
2. Do the features in the order of `feature.md` section 10.2 (features 08 to 14; feature 15 is
   last and waits for the proof of concept). Each feature's `feature.md` lists its tasks, what
   to read, and its pull requests. Inside a feature, follow the task order of section 10.3.
3. For each task:
   1. Read its section in the topic file.
   2. Write the code. Use the trait signatures in the design. If a signature must change, update
      the design file in the same commit, and say why.
   3. Write the tests that the task names, in the crate's `tests/` directory.
   4. Run `python3 scripts/ci/ci.py run <crate> --step build --step clippy --step test --step wasm`
      for each crate that you change.
   5. Update the task table in that feature's `feature.md` and the spec's `progress.md`. Add what you learned to
      `LEARNINGS.md`.
   6. Commit and push.
4. If a design choice is not clear, stop. Ask the owner with the options and your
   recommendation. Section 11 of `feature.md` has the questions that are still open.

## Rules to check before each push

- No `SystemTime::now()`, `Instant::now()`, `thread_rng()`, `getrandom` or similar in core
  code. Use the `Env` bundle. The clippy lint stops the build otherwise.
- No function-local `use` statements. No fully qualified paths in function bodies. Imports go
  at the top of the file.
- No `#[cfg(test)] mod tests` in source files. Tests go in `tests/`.
- No `todo!()` in code. The `todo!("Tn")` marks in the design files only show which task
  writes a body.
- Every new limit is a configuration field with a documented default.
