# Spec 77 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); virtual machine
(VM); pull request (PR); continuous integration (CI); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## Landed

- 2026-11-11: the spec, with ten features. It is a draft. No code exists. The owner has not
  yet answered the open questions (requirements section 9).
- 2026-11-11: the research. The Firecracker, libkrun (1.19 and `main`), Cloudflare
  sandbox-sdk and Cloudflare containers sources were read. The facts and their sources are
  in `learnings.md`.

## In progress

Nothing. The spec waits for the owner review.

## Next

1. The owner reads `requirements.md` and answers questions Q1 to Q9.
2. Update the spec with the answers. Set `status: "in-progress"`.
3. Feature 00 (one agent, sonnet).
4. Features 01, 02, 03 part A and 05 in parallel (one agent each).
5. Feature 04: milestone M1, the first sandbox, on libkrun.
6. Feature 03 part B, then feature 06: milestone M2, Firecracker.
7. Feature 07: version 0.1.
8. Features 08 and 09: version 0.2.

## Milestones

| Milestone | Features | State |
|---|---|---|
| M1: a command runs in a libkrun microVM on Linux and macOS | 00, 01, 02, 03 part A, 04 | not started |
| M2: a command runs in a Firecracker microVM on Linux | 03 part B, 05, 06 | not started |
| 0.1: `Provider` implementations and an example | 07 | not started |
| 0.2: Cloudflare Sandbox and Cloudflare Containers | 08, 09 | not started |

## Facts that a feature must measure

The research could not prove these from the sources. Each one is a "measure first" step in
its feature, and the result goes to `learnings.md`.

| Feature | Fact |
|---|---|
| 01 | The musl agent links on macOS with `rust-lld`. |
| 02 | How an `async fn` in this repository awaits readiness of a raw Unix socket. |
| 04 | `krun_add_virtiofs3` with the root tag and `read_only = true` boots with no `krun_set_root`. |
| 04 | `krun_set_exec`: `argv` has, or does not have, the program name. |
| 04 | The `libkrunfw` kernel has overlayfs, and overlayfs accepts `lowerdir=/` on virtio-fs. |
| 05 | The `gen_api` generator handles enum strings with a space and with a digit first. |
| 06 | The oldest Firecracker version that the real test passes on. |
| 08 | The Durable Object can open a raw connection to a container port (M1 to M5 of feature 08). |
| 09 | A WebSocket client exists in this repository. |

## Findings to file as GitHub issues

Root rule 9 says that each finding becomes an issue. The spec session filed none, because
the owner asked for design work only. File these when the spec is approved, or tell the
session to file them now.

| Finding | Label |
|---|---|
| `ResolvedPorts.ssh_port` and `vnc_port` are `u16`; a provider with no such port returns 0 (`foundation_deployment_platform/src/providers/mod.rs`). Likely fix: `Option<u16>`. | `severity: low` |
| `backends/foundation_deployment_docker/specs/swagger2openapi.py` is in one crate; a second Swagger 2.0 source (Firecracker) needs it. Likely fix: move it to `scripts/openapi/`. | `severity: low` |
| The `.agents` skill `specifications-management` requires `start.md` files; spec 76 and this spec do not have them. Likely fix: update the skill or add the files. | `severity: low` |
