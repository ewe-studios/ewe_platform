# Spec 77 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** remote procedure call (RPC); virtual machine (VM); Durable
Object (DO); minimum viable product (MVP); pull request (PR); continuous integration (CI);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## Landed

- 2026-11-11: the research. The Firecracker, libkrun (1.19 and `main`), libkrunfw,
  Cloudflare sandbox-sdk, Cloudflare containers and Rust `worker` crate sources were read.
  The facts and their sources are in `learnings.md`.
- 2026-11-11: the first design (one guest agent with an own frame protocol; a TypeScript
  bridge Worker). The owner replaced it the same day; `learnings.md` section 7 records why
  each part went.
- 2026-11-11: the owner's architecture. A control plane (`agent-sandbox`) and a slave in
  each sandbox (`agent-slave`) that talk over Connect RPC. All logic is library code in
  `foundation_platform_sandbox`, split into `shared`, `native` and `cloudflare`. Programs,
  the DO and the `Provider` implementations are boundaries. The MVP is Firecracker, then
  Cloudflare.
- 2026-11-11: the spec in the story-driven form: `requirements.md` and 16 feature files.
  Each feature has its story, bounded context, law worksheet, invariants, must-have lists,
  acceptance scenarios, technical breakdown and a table from laws to tests.
- 2026-11-12: a review by a fresh agent (a smaller model with no context, as the owner
  asked). It found design defects and gaps; the spec was changed for each. The main ones:
  a program start no longer needs `chroot` when the root is `/`; children are spawned from
  one long-lived thread, because the parent-death signal follows the spawning thread; a
  Durable Object stores its sandbox ID and restores before it handles an alarm; a manager
  has a drop policy, so a restart does not kill what `restore` must find; one boot
  deadline covers the launch, the readiness wait and `Prepare`; file paths resolve inside
  the program root with `openat2`; the slave is a child subreaper when it is not PID 1;
  `ControlService`, `SandboxRpcClient` and `check_bearer` moved to feature 02, so feature
  07 does not need feature 06.
- 2026-11-11: the skill `story-driven-specification` in the `.agents` repository (on its
  `master`), and `specifications-management` version 5.0, which makes that layout
  mandatory. The submodule pointer of this repository moved to that commit.

No product code exists. The spec is a draft in a pull request for the owner's review.

## In progress

Nothing. The spec waits for the owner review.

## Next

1. The owner reviews the PR and answers the open questions (requirements section 10.2).
2. Update the spec with the answers. Set `status: "in-progress"`.
3. Feature 00 (one agent).
4. Features 01, 02, 03 and 04 in parallel (one agent each).
5. Feature 05, then feature 06: MVP-A (Firecracker).
6. Feature 07 in parallel with 05 and 06 (it needs only 01 and 02); then feature 08:
   MVP-B (Cloudflare).
7. Phase 2: features 09 (libkrun), 10 (Docker), 11 (nested).
8. Phase 3: features 12 to 15 (images).

## Milestones

| Milestone | Features | Features deep (longest path) | State |
|---|---|---|---|
| MVP-A: a command runs in a Firecracker microVM through `agent-sandbox`, the library, RPC and a `Provider` | 00 to 06 | 4 (00, then one of 01 to 04, then 05, then 06) | not started |
| MVP-B: the same calls work against a Cloudflare Worker | 07, 08 | 2 more (07 can start after 01 and 02; 08 also needs 06) | not started |
| Phase 2: libkrun, Docker, nested sandboxes | 09, 10, 11 | 1 to 2 more each | not started |
| Phase 3: sandbox images | 12, 13, 14, 15 | 4 more | not started |

## What was checked while the spec was written, and what was not

Checked by a command in the specification session:

- `foundation_nativeapis` with the `fd` feature compiles for `x86_64-unknown-linux-musl`.
- `foundation_deployment_platform` does not compile for that target with default features
  (a C dependency through TLS).
- `foundation_deployment` depends on `foundation_db`.

Taken from source code and documents, with file and line in `learnings.md`: every
Firecracker, libkrun, libkrunfw, Cloudflare and `worker` crate fact.

Not run: no microVM, no container and no Worker was started. The session had no
`/dev/kvm`. The first real run of each driver is in its feature, and each feature's "Done
when" needs that run.

## Things a feature decides by a run

These are not open design questions. The design does not depend on the answer; the feature
records the value.

| Feature | Value to record |
|---|---|
| 05 | The boot time; the oldest Firecracker version that the real tests pass on. |
| 06 | The size of `agent-slave` for each target. |
| 08 | Whether `wrangler dev` keeps a container across a restart (the test accepts both). |
| 09 | The boot time on Linux and macOS; the libkrun version of each run. |
| 12 | Which hosts have a libkrun with block support. |

## Open questions for the owner

Known layout difference: features 03, 04 and 09 have the story, context, rules and
scenario sections with letters (A to D) between "1. Goal" and their numbered technical
sections; the other features use the nine numbered sections of the skill. The content is
the same. A renumber is a mechanical edit for a later pass.

Requirements section 10.2: Q1 (the meaning of "Docker sandbox" and "Cloudflare sandbox"),
Q2 (`foundation_platform_docker` or the existing `foundation_deployment_docker`), Q3
(where the `agent-slave` entry point is; the measurement says its own package), Q4
(authentication for the MVP), Q5 (no `start.md` files).

## Findings to file as GitHub issues

Decision D9: no issue is filed while the spec is in review. Feature 06, section 6.10, has
the list to file when the owner approves.
