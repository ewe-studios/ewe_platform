---
workspace_name: "ewe_platform"
spec_directory: "specifications/88-wasm-runtime-coverage"
this_file: "specifications/88-wasm-runtime-coverage/start.md"
created: 2026-11-15
---

# Start: Wasm Runtime Coverage

## What this specification is

It proves three layers on three wasm hosts:

- the layers: `valtron`, `foundation_http`, `foundation_connectrpc`;
- the hosts: embedded Deno, headless Chromium, and workerd through `wrangler dev --local`.

It also repairs the two executors that cannot report a failure today: the browser host and the
two wasm-bindgen hosts.

The work is in four features. Read this file, then read the feature you are given. Do not start
at the first feature file and read all four; each feature is self-contained and names its own
reading list.

## The features and their order

| Feature | Design | Brief | Order |
|---|---|---|---|
| 00-executors | [feature](./features/00-executors/feature.md) | [start](./features/00-executors/start.md) | 1: every other feature runs on the executors it repairs |
| 01-valtron-runtime | [feature](./features/01-valtron-runtime/feature.md) | [start](./features/01-valtron-runtime/start.md) | 2, parallel with 02 and 03 |
| 02-http-runtime | [feature](./features/02-http-runtime/feature.md) | [start](./features/02-http-runtime/start.md) | 3, parallel with 01 and 03 |
| 03-connectrpc-runtime | [feature](./features/03-connectrpc-runtime/feature.md) | [start](./features/03-connectrpc-runtime/start.md) | 4, parallel with 01 and 02 |

Features 01, 02 and 03 change different crates. Run them at the same time, one agent each.
Each feature folder holds a `feature.md` (the design) and a `start.md` (the self-contained
brief for its agent). Give the agent the `start.md` path; it reads the design from there.

## Agent workflow

1. Read `requirements.md` in full. Section 2 measures the state on 2026-11-15. Section 2.6 is
   the false-green defect and section 2.7 is the harness constraint. Section 4 records the three
   owner decisions. Section 6 holds rules R1 to R15; they apply to every feature.
2. Read `.agents/AGENTS.md`, `.agents/skills/main-agent-orchestration/skill.md` and
   `.agents/skills/code-verification/skill.md`.
3. Read the feature file you are given, in full, before you change a file.
4. Read `.agents/skills/rust-valtron-usage/skill.md` before a valtron case. Its test section is
   older than the code: a wasm case is `#[wasm_test]`, `#[valtron_wasm_test]`,
   `#[wasm_bindgen_test]` or `#[valtron_bindgen]`; `#[valtron_test]` is native only.
5. Work on one task at a time, in the order of the feature's section 7.
6. Report to the Main Agent after each task. Do not commit. The Main Agent reviews the report
   against the feature's "Done when" and commits.
7. Update the feature's notes, the spec's `progress.md` and `LEARNINGS.md` before you stop.

## Root rules that decide the hard cases

| Rule | Effect here |
|---|---|
| 4 | stream bodies, never buffer |
| 6 | async all the way; a waiting API is `async fn` |
| 7 | tests live in `tests/`; the owned cases sit in the lib for the reason in R9 |
| 8 | limits are configuration, not constants (#573) |
| 9 | every bug you find becomes a GitHub issue with a severity label |
| 10 | keep `progress.md` and `learnings.md` current |
| 12 | write in ASD-STE100 Simplified Technical English (STE) |
| 13 | no AI attribution in any commit, pull request or review message |
| 15 | every crate is tested on Linux, macOS and Windows, or it names a reason |

## Environment facts

- `git status` fails on the `whisper.cpp` symlink. Scope it by path.
- `gh` needs `env -u GITHUB_TOKEN gh`.
- Stop a process by its saved PID, never with `pkill -f`.
- The wasm tasks need `MISE_ENV=wasm`, which selects `mise.wasm.toml`.
- The local cargo profile variables set a codegen backend. A crate outside the workspace needs
  `env -u CARGO_PROFILE_DEV_CODEGEN_BACKEND -u CARGO_PROFILE_UAT_CODEGEN_BACKEND` when it does
  not define that profile.

## Workflow

Requirements → Select task → Implement → Report → Verify → Commit → Update progress

---

_Created: 2026-11-15_
