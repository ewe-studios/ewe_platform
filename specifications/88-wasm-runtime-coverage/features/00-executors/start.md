---
feature: "Executors"
this_file: "specifications/88-wasm-runtime-coverage/features/00-executors/start.md"
spec: "specifications/88-wasm-runtime-coverage/requirements.md"
model: "sonnet"
branch: "spec/split-65"
created: 2026-11-15
---

# Start: Executors

You are one agent. You own feature 00 of spec 88. Read this file, then read the feature file
and the requirement sections it names. Do not read the other three features; they start after
yours.

## Mission

Five wasm test executors must report the truth, and a pull request must run all of them. Two of
them (the browser host and the two wasm-bindgen hosts) cannot run or cannot fail today. You
repair them. You also give every host a local endpoint, because every suite after yours needs
one.

You do not add layer tests. Features 01, 02 and 03 do that.

## Required reading, in this order

1. `specifications/88-wasm-runtime-coverage/requirements.md` sections 2.1, 2.2, 2.6, 2.7, 4, 6
   and 9. Skip the rest on the first pass.
2. `features/00-executors/feature.md` in full. Sections 6.1 to 6.10 are the task briefs.
3. `.agents/skills/code-verification/skill.md` and `.agents/skills/rust-valtron-usage/skill.md`
   (the second only for the parts about `wasm_test` and `valtron_wasm_test`).
4. The files each task names, before you change them.

## Owned paths

You write only these:

- `backends/foundation_testbed/src/` (the `wasm` module, the `wasm-testbed` binary, the
  templates)
- `backends/foundation_browser/src/` and `backends/foundation_browser/tests/`
- `mise.toml`, `mise.wasm.toml`
- `scripts/ci/ci.py`, `ci/packages.toml`, `.github/workflows/check.yaml`
- `backends/foundation_testbed/integration/bindgen_failing_case/` (a new crate)
- `backends/foundation_testbed/integration/package.json` (delete only; T1)

You do not write `foundation_core`, `foundation_http` or `foundation_connectrpc`. If a task
needs a change there, stop and report it.

## Task order

Work T1 to T8 in that order. Each task has its own "Done when" row in feature section 6.1.
Run the verify command of a task before you start the next one. A task that cannot pass its
verify command is a report, not a workaround.

| Task | One line |
|---|---|
| T1 | rename `test:wasm-testbed:web` to `:browser`, delete `setup:playwright` and `integration/package.json`, make `test:browsers:chromium` portable and idempotent |
| T2 | complete the browser search order and make it public |
| T3 | replace the `node`/`npm`/`npx` preflight with a browser check |
| T4 | measure #569 with frame tracing, fix the reader contract, move the limits to configuration |
| T5 | read the wasm-bindgen verdict in both bindgen templates and both runners |
| T6 | one `/echo` endpoint per host, and `__FWT_BASE_URL__` |
| T7 | `--filter` for wrangler, one test artifact or a typed error, correct the stale comments |
| T8 | CI runs the five executors and the failing fixture, and the macOS browser job runs |

T4 is a diagnosis task. Do not guess the cause of #569. Add the trace, run it on macOS, read
the frames, write what you found in the issue and in `LEARNINGS.md`, then fix it. The reader
contract in feature section 6.5 is fixed whether or not it is the cause. Trace as far as the
evidence allows, and present the root cause when the fix is not small: a root cause in the
protocol layer, the transport or the frame parser stops T4 with a report. T5 to T8 must not wait
for an open protocol fix.

T6 has one open question: whether workerd permits a fetch to the dev server's own origin.
Measure it. Use the route if it works, the service binding if it does not, and record which
one in `LEARNINGS.md`.

## Rules that decide your choices

| Rule | What it means for you |
|---|---|
| R4 | every wait has a limit; a hung case fails the run, it does not hang it |
| R7 | write down the host fact behind an assertion: a citation or a measured line |
| R12 | every suite stays runnable case by case |
| R14 | each runner must be proven able to fail. T5's fixture is that proof |
| root 7 | no `#[cfg(test)] mod tests` in a source file; new tests go in `tests/` |
| root 8 | a size or a time is a config field with a documented default, never a `const` |
| root 9 | a bug you find becomes a GitHub issue with a `severity:` label. Report it; do not stop the feature for a low one |
| root 12 | write in ASD-STE100 Simplified Technical English (STE) |
| root 13 | no AI attribution anywhere |
| owner | stop a process by the saved PID, never `pkill -f` |

A public API you add to `foundation_browser` needs a version bump and a bump of every
consumer's requirement in the same change (owner rule on new public API).

## Commands you will need

```bash
cargo test -p foundation_browser --features browser-tests --test smoke
cargo run -p foundation_testbed --features wasm --profile uat --bin wasm-testbed -- --help
MISE_ENV=wasm mise run test:wasm-testbed:deno
mise run test:wasm-testbed:browser
python3 scripts/ci/ci.py plan --package wasm-tests
```

You are on a Mac. Run the browser suite here with `mise run test:wasm-testbed:browser`. It needs
no Docker container. The macOS and Windows CI jobs run on the full runs only (a push to `master`
or `platform/release`, a pull request into or from `platform/release`, the nightly schedule and a
manual run).

The local cargo profile sets a codegen backend from the environment, so a crate outside the
workspace needs:

```bash
env -u CARGO_PROFILE_DEV_CODEGEN_BACKEND -u CARGO_PROFILE_UAT_CODEGEN_BACKEND cargo build …
```

`git status` fails on the `whisper.cpp` symlink. Scope it by path.

## Do not

- commit to `spec/split-65`. You work in your own git worktree, on the branch the worktree
  starts on. Commit your work there in small steps and report the branch name. Do not push and
  do not switch branches. The Main Agent reviews your report and merges your branch.
- do not touch a crate another agent owns.
- do not leave a stub for a later feature. If a task needs a dependency, finish the dependency
  in the same task.
- do not weaken a test to make a runner pass. Report the runner.

## Report

Send one report. Keep it short; no full logs.

1. Per task: the files changed, the command you ran and its result (one line each).
2. The measured #569 root cause, with the frame evidence.
3. The T6 answer: self-fetch or binding.
4. Every new issue you filed or want filed, with its severity.
5. What you did not finish, and why.
6. The diff, as `git diff --stat` for your owned paths.

---

_Created: 2026-11-15_
