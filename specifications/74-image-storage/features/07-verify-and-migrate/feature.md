---
feature: "Verify the full path against the real services and migrate"
status: "planned"
priority: "medium"
depends_on: ["03-registry-backend", "04-vm-disk-store", "05-ci-wiring-and-docs", "06-cost-report"]
crates: []
issues: []
model: "sonnet"
branch: "spec74/f07-verify-and-migrate"
---

# Feature 07: Verification against the real services and migration

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** continuous integration (CI); Docker Hub (DH); gigabyte (GB);
gibibyte (GiB); GitHub Actions (GHA); GitHub Container Registry (GHCR); Hypertext Transfer
Protocol (HTTP); Open Container Initiative (OCI); pull request (PR); virtual machine (VM);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## 1. Goal

Prove the design against the real services, measure the facts that the spec assumes and
record them. Then migrate the images if the owner chooses GHCR. Do not leave an assumption
unmeasured.

## 2. Measured state

1. Feature 01–06 build the code and the wiring. No test has touched the real R2 bucket.
2. The 10 GB per-layer claim for Docker Hub is from third-party reports. Docker Hub does not
   publish it in its pricing page.
3. The Docker Hub pull limit is 100 per 6 hours anonymous and 200 per 6 hours with a free
   login. The current full `check.yaml` matrix has about 40 package jobs, each pulling the
   image once or more.
4. The real disk sizes and the real upload times are estimates.
5. Ten old Docker Hub repositories from 2021 are not used by this repository.

## 3. Requirements

1. **Real R2 compatibility:** run the client against the real bucket for these cases:
   - 64 MiB round trip;
   - 6 GiB single object with 64 MiB parts (above the 5 GiB single-PUT limit);
   - an interrupted upload and a resume;
   - a presigned GET;
   - a delete.
   Each case verifies the SHA-256 and deletes its objects.
2. **Disk round trip:** upload one real Windows disk archive and one real macOS archive (if
   the Apple host is available) and download each to an empty directory. Record the
   compressed size, the upload time, the download time and the SHA-256 check result.
3. **Docker Hub layer sizes:** for each image tag in one full set, print the layer sizes with
   a tool that reads the manifest (`docker manifest inspect`, `crane manifest` or
   `regctl manifest get`). Record the largest layer. Confirm it is below 10 GB, or record the
   failure.
4. **Pull-rate measurement:** run a full `check.yaml` matrix on the selected registry. Sum the
   `exists` retry lines and the anonymous pull failures. Record the count against the 200 per
   6 hour budget. If the budget is close, switch the default to GHCR.
5. **Cost measurement:** read the real month-to-date cost from the Cloudflare dashboard and
   compare it with the report estimate. Record the difference and its cause.
6. **Migration (only when the owner chooses GHCR):**
   - promote every tag of one full set to GHCR with `docker pull`, `docker tag`, `docker
     push` (or `skopeo copy`);
   - set every package to public;
   - run the full matrix on GHCR;
   - change the default backend in `config.toml`;
   - keep Docker Hub read-only for 30 days; record the rollback command.
7. **Cleanup list:** list the unused Docker Hub repositories with their last update date. Give
   the list to the owner. Do not delete a repository without owner approval.
8. Update `progress.md` with every measurement and `learnings.md` with every surprise.

## 4. Design

### 4.1 Real-service workflow

`.github/workflows/storage-r2-smoke.yaml` gains a `case` input:

| Case | Size | Runtime (estimate) |
| --- | --- | --- |
| `small` | 64 MiB | under 1 minute |
| `large` | 6 GiB | 10–20 minutes |
| `resume` | 512 MiB | 2–5 minutes |
| `disk` | the newest disk archive | depends on the size |

The nightly schedule runs `small`. The manual dispatch runs any case. `large` and `disk` are
manual only, because of the time and the storage cost.

### 4.2 Measurement record

`progress.md` gains a table:

| Measurement | Value | Date | Command |
| --- | --- | --- | --- |
| Largest layer, default flavor | | | |
| Largest layer, CUDA flavor | | | |
| Docker Hub retries in one matrix | | | |
| Anonymous pull failures | | | |
| R2 6 GiB upload time | | | |
| R2 6 GiB download time | | | |
| Windows disk compressed size | | | |
| macOS disk compressed size | | | |
| Report estimate vs invoice | | | |

### 4.3 Rollback

Docker Hub stays unchanged during the migration. The rollback is one config change:

```toml
[storage]
backend = "dockerhub"
```

The workflows pull by digest when the image exists, so a rollback does not rebuild.

## 5. Files

| File | Change |
| --- | --- |
| `.github/workflows/storage-r2-smoke.yaml` | `case` input and the four cases. |
| `ci/platform-images/tests/r2_tests.nu` | Any test the real service proves wrong. |
| `ci/ewe-platform-image/config.toml` | The chosen backend, only when the owner chooses. |
| `specifications/74-image-storage/progress.md` | The measurement table, filled. |
| `specifications/74-image-storage/learnings.md` | Surprises and root causes. |
| `docs/bug-backlog.md` | Only if a finding needs a long entry; the issue is the record. |

## 6. Tasks

- [ ] Add the `case` input and the four cases to the smoke workflow.
- [ ] Run `small`, `resume` and `large` against the real bucket. Record the results.
- [ ] Run the disk round trip for Windows; for macOS when the Apple host is free.
- [ ] Measure the largest layer of every image in one full set.
- [ ] Run a full matrix on the selected registry and record the pull budget.
- [ ] Compare the report estimate with the dashboard invoice.
- [ ] Migrate to GHCR only when the owner chooses; otherwise record the rollback command.
- [ ] List the unused Docker Hub repositories and give the list to the owner.
- [ ] Fill the measurement table and `learnings.md`.

## 7. Tests

The checks are the workflow runs and the recorded values. There is no new unit test unless a
real-service run proves a local test wrong; then that test changes and gets a note.

## 8. Verification

```bash
# Manual, with secrets in the environment.
mise run storage:r2-smoke -- --case small
mise run storage:r2-smoke -- --case resume
mise run storage:r2-smoke -- --case large
mise run storage:r2-smoke -- --case disk
```

```bash
# Layer sizes for the default flavor.
docker pull ewestudios/ewe-platform-ci:latest
docker manifest inspect ewestudios/ewe-platform-ci:latest | \
  python3 -c "import sys,json; m=json.load(sys.stdin); print(sum(l['size'] for l in m['layers']))"
```

## 9. Done when

- The real R2 cases pass and the measurements are in `progress.md`.
- The largest Docker Hub layer is recorded, with the command and the date.
- The pull budget is measured; the default registry is the one the measurement supports.
- The cost estimate is compared with the invoice.
- The unused repository list is with the owner.
- `progress.md` and `learnings.md` are current.

## 10. Risks

| Risk | Control |
| --- | --- |
| The 6 GiB case costs more than expected. | Delete the object in the same run; the cost is minutes of storage. |
| A real-service run finds a defect late. | Run the `small` and `resume` cases first; they cover the client. |
| The matrix run mixes two registry backends. | One backend per run; the plan prints it at the top. |
| The migration leaves a private package. | The visibility check from feature 03 runs on every tag. |
| Cleanup deletes a used image. | The owner approves each deletion; the list has the pull count and the date. |
