---
workspace_name: "ewe_platform"
spec_directory: "specifications/74-image-storage"
this_file: "specifications/74-image-storage/start.md"
created: 2026-11-08
---

# Start: Spec 74, low-cost storage for CI images and VM disks

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** continuous integration (CI), Cloudflare R2 (R2), GitHub
Container Registry (GHCR), Open Container Initiative (OCI), pull request (PR), virtual
machine (VM), AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100.

You implement **one feature** of this spec. The main session gave you its number.

## Read first, in this order

1. `CLAUDE.md` and `.agents/AGENTS.md` at the repository root.
2. `specifications/CLAUDE.md`.
3. [`requirements.md`](./requirements.md): sections 6 (data model), 7 (configuration and
   secrets), 9 (rules), 10 (verification) and 11 (done). They apply to every feature.
4. Your feature file: `features/NN-<slug>/feature.md`. Read all of it before you change code.
5. `ci/platform-images/README.md`, `ci/ewe-platform-image/README.md`,
   `ci/platform-images/scripts/images.nu` and `ci/platform-images/scripts/vm.nu`.
6. [`progress.md`](./progress.md) and [`learnings.md`](./learnings.md).

## Workflow

1. Do not start until the main session hands off the feature and the owner has accepted the
   decision in section 13 of `requirements.md` that the feature needs.
2. Create your branch from `master`, named `spec74/fNN-<slug>`.
3. Do the tasks in the order the feature file gives. For each behaviour: write the test, see
   it fail on the old code, implement, see it pass.
4. Run the feature's verification commands exactly. Record the results.
5. Tick the tasks in your feature file. Add a short entry to `progress.md`. Add what you
   learned to `learnings.md`. Open or update a GitHub issue for every finding, with a
   `severity:` label.
6. Commit, push, report to the main session. Do not merge.

## Stop and report, do not guess

Stop and report to the main session when:

- the design in the feature file does not fit the code (give the file, the line and the
  reason);
- a change is needed outside the feature's named files and is larger than a few lines;
- a real Cloudflare R2 call behaves differently from the S3 documentation;
- the owner decision that the feature needs is missing;
- a test fails and you cannot find the root cause in a reasonable time.

## Lane order

The main session starts features in the order in `requirements.md` section 8.1. At most three
agents run at once. Features 01 and 02 own the same directory; feature 02 waits until 01
merges.

| Lane | Feature | Work | Area |
| --- | --- | --- | --- |
| 1 | 01 storage configuration and manifest | design and code | ci/platform-images, ci/ewe-platform-image |
| 2 | 02 R2 object client | design and code | ci/platform-images/scripts |
| 3 | 03 registry backend | design and code | ci/platform-images/scripts, workflows |
| 4 | 04 VM disk store | design and code | ci/platform-images/scripts, actions/boot-vm |
| 5 | 05 CI wiring and documentation | mechanical | .github/workflows, docs |
| 6 | 06 cost report and lifecycle | mechanical | ci/platform-images/scripts, docs |
| 7 | 07 verification and migration | design and measured | tests, .github/workflows |

## No secrets in a feature branch

Do not ask for a Cloudflare token in a PR. Use MinIO for the local tests. The real-R2 checks
run in a workflow on the main repository, where the secrets exist. A feature that cannot
complete without a real bucket stops and reports.
