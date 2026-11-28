---
workspace_name: "ewe_platform"
spec_directory: "specifications/completed/71-durable-streams-in-valtron"
this_file: "specifications/completed/71-durable-streams-in-valtron/start.md"
created: 2026-10-30
---

# Start: Spec 71, durable streams

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** AeroSpace and Defence Industries Association of Europe (ASD), the
owner of the STE standard ASD-STE100.

You implement **one feature** of this spec. The main session gave you its number.

## Read first, in this order

1. `CLAUDE.md` and `.agents/AGENTS.md` at the repository root.
2. `.agents/skills/rust-valtron-usage/skill.md` and `.agents/skills/rust-clean-code/skill.md`.
3. [`requirements.md`](./requirements.md): sections 2 (the contract), 3 (architecture), 5
   (rules) and 6 (verification). Read section 3.0 first. It separates the core from valtron.
4. The [Durable Streams research note](../../research/edge-data/durable-streams.md). It is a
   summary. The protocol and `requirements.md` are the contract. The note omits `Stream-Seq`,
   the client and the JSON wire rules.
5. Your feature file: `features/NN-<slug>/feature.md`. Read all of it before you change code.
6. [`progress.md`](./progress.md) and [`learnings.md`](./learnings.md).

## The three rules that matter most

1. **Conformance first.** The protocol core must pass the reference server suite and the
   reference client suite without valtron. Do not hide a protocol rule in the valtron layer.
2. **The core is portable.** The core has no valtron, HTTP, SSE, ETag or CDN type. The adapters
   own the wire. The valtron layer owns scheduling.
3. **HTTP is one surface, not the server.** The core must run in a local embedded process, a DO,
   a Worker, a WebSocket server and the `foundation_ai` Session. Tests prove each target.

## Workflow

1. Create your branch from `master`.
2. Do the tasks in the order the feature file gives. Write the test first, see it fail, then make
   it pass.
3. Run the feature's verification commands exactly. Record the results.
4. Tick the tasks in your feature file. Add a short entry to `progress.md`. Add what you learned
   to `learnings.md`.
5. Commit, push, open the PR. The PR body has: what changed, the before/after result of each test,
   the flake-check counts, the commands you ran and their results.
6. Report to the main session. Do not merge.

## Stop and report, do not guess

Stop and report to the main session when:

- the design in the feature file does not fit the code (give file, line and the reason);
- a fix needs a change outside the feature's named files that is larger than a few lines;
- a test fails and you cannot find the root cause in a reasonable time;
- the disk runs low and `cargo clean -p` on finished packages does not free enough.

---

**Workflow:** Requirements → Select feature → feature.md → skills → research note → ONE TASK AT A
TIME → test-first → verify → report → review.

---

_Created: 2026-10-30_
