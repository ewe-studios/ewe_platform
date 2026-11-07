---
feature: "The Free runner disk step is bounded and deletes only known paths"
status: "pending"
priority: "medium"
depends_on: []
crates: []
issues: [194]
model: "haiku"
branch: "spec72/f12-ci-free-runner-disk"
---

# Feature 12: CI free-runner-disk step

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** continuous integration (CI); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Stop the "Free runner disk" step from stalling a job without a bound. Close issue
[#194](https://github.com/ewe-studios/ewe_platform/issues/194).

## 2. Problem

`.github/workflows/check.yaml` runs "Free runner disk" at lines 121 and 308. The command is
`find /runner-free -mindepth 2 -maxdepth 2 -exec rm -rf {} +` (lines 124 and 311), with no
time bound. On 2026-11-05 run 42048334017 cancelled the `foundation_buildtools` job at this
step, and run 42040473182 left the `foundation_wireguard` job there for over an hour, so the
whole run never completed.

## 3. Requirements

1. Delete only the paths the job owns. Do not walk the whole runner tree.
2. Print `du -sh` before and after, so the log shows the effect.
3. Bound the step with `timeout-minutes` and a `timeout` command around the deletion.
4. A timeout is a visible failure, not a silent success.
5. Keep both occurrences in step.

## 4. Design

Replace the `find ... -exec rm -rf` command with an explicit list of the directories the job
frees (for example the job's cargo target and the cache directory it creates). Wrap it:

```yaml
- name: Free runner disk
  timeout-minutes: 10
  run: |
    du -sh <paths> || true
    timeout 480 rm -rf <paths>
    du -sh <paths> || true
```

The exact paths are the ones the job writes. Read the job's earlier steps to list them.

## 5. Tasks

- [ ] Read both occurrences and the job steps around them.
- [ ] List the paths each job owns.
- [ ] Replace the command and add the bounds.
- [ ] Run the workflow in a pull request and read the timing.
- [ ] Close #194 in the PR body.

## 6. Tests

The CI job is the test. Report the step duration and the `du -sh` lines.

## 7. Verify

```bash
# Local syntax check of the workflow, if a linter is available:
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/check.yaml'))"
# The real proof is the pull request's workflow run.
```

## 8. Done when

- No unbounded `find ... -exec rm -rf` remains in `check.yaml`.
- The step has a time bound and prints the before and after size.
- The pull request run shows the step finishing quickly.
- Issue #194 is closed by the PR.
- `progress.md` records the step duration.
