---
description: "Close the 17 open medium-severity issues left after spec 70: inject the clock into the HTTP app, the Docker container config and the toolings sleeper, bound the background registry shutdown, fix the wireguard hole-punch panic, the WASI clock read, the netio load rate, six ptrace and vfs-preload dispatch defects, the keychain macOS flake, and add the two missing CI jobs. Also fixes the concurrent Turso read failure found during the keychain work, and removes a fixed sleep from that test's helper."
status: "planned"
priority: "high"
created: 2026-11-07
updated: 2026-11-07
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "medium"
  tags: [clock, injection, ptrace, vfs, wireguard, wasm, netio, http, docker, toolings, keychain, ci, valtron]
  stack_files: [rust.md]
  skills: [rust-clean-code, rust-valtron-usage]
  tools: []
has_features: true
has_fundamentals: false
builds_on: "specifications/completed/70-foundational-io-hardening"
related_specs:
  - "specifications/completed/70-foundational-io-hardening"
  - "specifications/71-durable-streams-in-valtron"
features:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# Spec 72: Medium-severity defect fixes

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); continuous integration
(CI); file descriptor (fd); input/output (I/O); pull request (PR); random number generator (RNG);
virtual file system (VFS); WebAssembly (Wasm); WebAssembly System Interface (WASI); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Why this spec exists

Spec 70 closed the high-severity findings. Its audit left 17 open issues with the
`severity: medium` label. Each one is a real defect: a wrong result on an edge case, an
unbounded wait, a silent fallback to a real clock, a hot loop, or a missing CI job. None is
a new feature. This spec fixes exactly these 17 issues and closes each one in its change.

The issues come from three groups:

1. **Injected clocks (rule 11).** `foundation_http`, foundation_deployment_platform's Docker
   config, `foundation_toolings`, and the WASI host still read the host clock or fall back to
   `fork_ctx` in a path that must take its clock from the `Ctx` bundle.
2. **Correctness in the native VFS and netio.** The ptrace syscall dispatch and the
   vfs-preload shim mishandle six edge cases, `LoadTracker::current_rate` is always zero, and
   the wireguard relay session panics.
3. **CI and a flaky test.** Two CI jobs are missing or unbounded, and one keychain test fails
   on macOS.

The owner added two features on 2026-11-07, after the spec started:

- **Feature 13** fixes a `severity: high` issue found while fixing feature 10. Two concurrent
  Turso statements on one connection fail with `concurrent use forbidden`. The owner asked
  for it to be added and fixed at once.
- **Feature 14** fixes an issue found in the same work. A test helper sleeps 150 ms before its
  first request. The owner asked for it to be the last feature of the spec.

## 2. Scope

The scope was the 17 open `severity: medium` issues as of 2026-11-07. The owner then added
#357 (high) and #358 (low) on the same day. Do not fix any other low-severity issue. Do not add
any other feature.

| Issue | Title (short) | Feature |
| --- | --- | --- |
| #234 | `foundation_http` constructors build a bag from the real clock | 01 |
| #233 | `shutdown_within` does not pass its deadline to the background registry | 02 |
| #223 | wireguard `RelaySession` panics on a failed hole punch with no relay | 03 |
| #220 | Docker `ContainerConfig` falls back to `fork_ctx` and panics without a pool | 04 |
| #219 | WASI extern-C read is a marked direct clock read | 05 |
| #214 | netio `LoadTracker::current_rate` is always zero | 06 |
| #205 | ptrace open dispatch reads the mode from the flags argument | 07 |
| #206 | ptrace dup2/dup3 dispatch ignores close-on-exec | 07 |
| #207 | ptrace dup2/dup3 dispatch accepts any new fd number | 07 |
| #208 | ptrace write dispatch ignores `O_APPEND` | 07 |
| #209 | the ptrace loop ignores `PTRACE_SETREGS` failures | 08 |
| #210 | vfs-preload `ftruncate` turns a negative length into `usize::MAX` | 08 |
| #201 | `ProjectBuilderTask` never wakes and never pops `change_queue` | 09 |
| #203 | `DurationSleeper` reads the wall clock instead of the injected `Clock` | 09 |
| #197 | `slow_request_body_does_not_hold_worker` fails on macOS | 10 |
| #196 | mobile targets have no CI build | 11 |
| #194 | the "Free runner disk" step stalls jobs | 12 |
| #357 | two concurrent Turso reads fail with `concurrent use forbidden` | 13 |
| #358 | the streamed-body helper sleeps 150 ms instead of synchronizing | 14 |

## 3. Rules

These come from `CLAUDE.md`, `.agents/AGENTS.md` and `specifications/CLAUDE.md`. Read all
three before you start.

1. **Tests live in `tests/`**, registered in `tests/**/mod.rs`. No inline `#[cfg(test)]`
   module (root rule 7). If a file you touch has one, move it in the same change.
2. **Side effects are injected, never global** (root rule 11). A clock, an RNG, a transport
   or a storage handle comes from the `Ctx` bundle. No `SystemTime::now()`, no
   `Instant::now()` and no `thread_rng()` in core logic. A missing bundle is a typed error.
3. **Limits are configuration** (root rule 8). A size cap, a timeout or a batch size is a
   configured field with a documented default that names its source. Never a `const` in the
   code path.
4. **Every bug found becomes a GitHub issue with a severity label** (root rule 9), in the same
   change, and the issue closes in the change that fixes it. This spec adds no feature beyond
   #357 and #358; a finding outside these 19 issues is a new issue, not new spec scope.
5. **Stream bodies, never buffer them** (root rule 4). Check the status before you touch a
   body.
6. **Async all the way** (root rule 6). A public API that waits is `async` or a valtron
   readiness. Never a synchronous loop that `continue`s on `Wait`.
7. **Never skip, ignore or disable a test** to get green. Never lower a timeout to hide a
   hang.

## 4. Verification protocol

Set these first. They keep the build inside the disk budget.

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
```

Each feature names its exact commands. The shape is:

```bash
cargo fmt -p <crate>
cargo clippy --profile uat -p <crate> --all-targets -- -D warnings
cargo test --profile uat -p <crate>
```

- A test that can hang has a time limit inside it.
- A test that uses threads, sockets or timing passes 20 runs in a row. Report the count.
- A test that proves a defect has a regression version that fails on the old code. Record the
  before and after result in the PR body.
- Full CI is the PR's GitHub run. Local runs cover only the changed crates.

## 5. Features

| # | Feature | Issues | Crate or area | Model |
| --- | --- | --- | --- | --- |
| 01 | [HTTP app clock injection](./features/01-http-app-clock/feature.md) | #234 | foundation_http | sonnet |
| 02 | [Bounded background shutdown](./features/02-core-background-shutdown-deadline/feature.md) | #233 | foundation_core | sonnet |
| 03 | [Wireguard hole-punch error](./features/03-wireguard-hole-punch/feature.md) | #223 | foundation_wireguard | sonnet |
| 04 | [Docker container clock](./features/04-docker-container-clock/feature.md) | #220 | foundation_deployment_platform | sonnet |
| 05 | [WASI time source](./features/05-wasm-wasi-time-source/feature.md) | #219 | foundation_wasm | sonnet |
| 06 | [Load tracker rate](./features/06-netio-load-tracker-rate/feature.md) | #214 | foundation_netio | sonnet |
| 07 | [Ptrace dispatch correctness](./features/07-nativeapi-ptrace-dispatch/feature.md) | #205, #206, #207, #208 | foundation_nativeapis | sonnet |
| 08 | [Ptrace setregs and truncate](./features/08-nativeapi-setregs-and-truncate/feature.md) | #209, #210 | foundation_nativeapis | sonnet |
| 09 | [Toolings clock and wake](./features/09-toolings-clock-and-wake/feature.md) | #201, #203 | foundation_toolings | sonnet |
| 10 | [Keychain macOS flake](./features/10-keychain-macos-flake/feature.md) | #197 | foundation_keychain | sonnet |
| 11 | [CI mobile target builds](./features/11-ci-mobile-target-builds/feature.md) | #196 | `.github/workflows` | haiku |
| 12 | [CI free-runner-disk step](./features/12-ci-free-runner-disk/feature.md) | #194 | `.github/workflows` | haiku |
| 13 | [Concurrent Turso reads](./features/13-db-turso-concurrent-reads/feature.md) | #357 | foundation_db | sonnet |
| 14 | [Streamed-body start synchronization](./features/14-keychain-start-server-sync/feature.md) | #358 | foundation_keychain | haiku |

Order: 01–06 and 10–12 are independent. Feature 07 runs before 08 (the same crate and the
same test module). Feature 13 is independent and has high priority. Feature 14 is the last
feature. Run at most three agents at a time.

## 6. Done for the whole spec

- [ ] Every issue in section 2 is closed, and its closing PR body names it.
- [ ] No changed source file reads the host clock outside a host entry point; `rg -n
      'SystemTime::now|Instant::now|fork_ctx'` in the changed crates shows only the boundary.
- [ ] The two CI jobs run in the pull request and pass: the mobile checks, and the bounded
      disk-free step.
- [ ] A regression test exists for each defect, and it failed on the old code.
- [ ] `progress.md` and `learnings.md` are current.

## 7. Out of scope

- Any `severity: low` issue other than #358.
- New API surface beyond what a fix needs.
- A redesign of the ptrace dispatch or the VFS shim.
- A mobile release pipeline; feature 11 only compiles the targets.
