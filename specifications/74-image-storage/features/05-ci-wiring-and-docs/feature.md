---
feature: "Wire the storage into the workflows and document it"
status: "planned"
priority: "medium"
depends_on: ["03-registry-backend", "04-vm-disk-store"]
crates: []
issues: []
model: "haiku"
branch: "spec74/f05-ci-wiring"
---

# Feature 05: CI wiring and documentation

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Amazon Web Services (AWS); Cloudflare R2 (R2); continuous
integration (CI); GitHub Actions (GHA); GitHub Container Registry (GHCR); pull request (PR);
virtual machine (VM); YAML Ain't Markup Language (YAML); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Connect the storage code to the workflows and to the repository settings. Add the AWS CLI to
the images. Write the operator documentation. This feature adds no logic; it wires and
documents what features 01–04 built.

## 2. Measured state

1. `.github/workflows/platform-images.yaml` has a registry login and a build-push step. It
   knows no storage backend and no R2.
2. `.github/workflows/platform-vms.yaml` has no `disk-archive` and no disk publish job.
3. No workflow or image has the AWS CLI. `mise.toml` pins the tools that the images install.
4. `ci/platform-images/README.md` describes the registry and the persistent disk, not R2.
5. There are no repository secrets or variables for R2.

## 3. Requirements

1. The workflows use the plan outputs from feature 03. No workflow contains a registry host,
   a bucket name or a token name as a literal, except the new R2 secrets table.
2. Add `.github/workflows/storage-r2-smoke.yaml`:
   - triggers: `workflow_dispatch` and a nightly `schedule`;
   - runs `mise run storage:r2-smoke` on `ubuntu-latest`;
   - uses `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_BUCKET`;
   - fails on the main repository when a secret is missing, and skips with a message on a
     fork;
   - deletes every object it creates, even on failure.
3. `platform-vms.yaml` passes `disk-archive: auto` and a `windows-publish-disk` job as feature
   04 defines.
4. `mise.toml` pins `aws-cli` and `zstd`. The pins have a comment that names the versions.
   Regenerate `mise.lock` if the repository uses one for these tools.
5. Documentation:
   - `ci/platform-images/README.md`: a new "Image and disk storage" section with the stores,
     the secrets, the licence rules, the retention and the costs;
   - `ci/ewe-platform-image/README.md`: the `[storage]` config fields;
   - `ci/platform-images/README.md`: the exact steps to create the R2 bucket, the scoped
     token and the lifecycle rule;
   - a costs table copied from `requirements.md` section 3.3, with the date of the prices.
6. The command-line help (`nu ci/platform-images/scripts/vm.nu disk --help` and the `r2.nu`
   doc comments) matches the README.

## 4. Design

### 4.1 Secrets and variables

| Name | Kind | Value |
| --- | --- | --- |
| `R2_ACCOUNT_ID` | Variable | The Cloudflare account identifier. |
| `R2_ACCESS_KEY_ID` | Secret | Read-write token for the one bucket. |
| `R2_SECRET_ACCESS_KEY` | Secret | Same token. |
| `R2_BUCKET` | Variable | `ewe-platform-ci`. |
| `R2_READ_ONLY_ACCESS_KEY_ID` | Secret | Read-only token, copied to the Apple host only. |
| `R2_READ_ONLY_SECRET_ACCESS_KEY` | Secret | Same token. |
| `PLATFORM_IMAGES_STORAGE_BACKEND` | Variable | `dockerhub` or `ghcr`. |
| `GITHUB_TOKEN` | Built in | GHCR push and pull. |

The README gives the token policy: one bucket, object read and write, no account-level
permission, rotate every 90 days.

### 4.2 Smoke workflow

```yaml
name: Storage (R2 smoke)
on:
  workflow_dispatch:
  schedule:
    - cron: "17 4 * * *"
permissions:
  contents: read
jobs:
  smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4.4.0
      - uses: ./ci/platform-images/actions/setup-nu
      - name: Check the secrets
        id: check
        env:
          R2_ACCOUNT_ID: ${{ vars.R2_ACCOUNT_ID }}
          R2_ACCESS_KEY_ID: ${{ secrets.R2_ACCESS_KEY_ID }}
          R2_SECRET_ACCESS_KEY: ${{ secrets.R2_SECRET_ACCESS_KEY }}
        run: |
          if [ -z "$R2_ACCOUNT_ID" ] || [ -z "$R2_ACCESS_KEY_ID" ] || \
             [ -z "$R2_SECRET_ACCESS_KEY" ]; then
            if [ "$GITHUB_REPOSITORY" = "ewe-studios/ewe_platform" ]; then
              echo "::error::the R2 secrets are missing"; exit 1
            fi
            echo "::warning::no R2 secrets; skipping"; echo "run=0" >> "$GITHUB_OUTPUT"; exit 0
          fi
          echo "run=1" >> "$GITHUB_OUTPUT"
      - name: Smoke
        if: steps.check.outputs.run == '1'
        env:
          R2_ACCOUNT_ID: ${{ vars.R2_ACCOUNT_ID }}
          R2_ACCESS_KEY_ID: ${{ secrets.R2_ACCESS_KEY_ID }}
          R2_SECRET_ACCESS_KEY: ${{ secrets.R2_SECRET_ACCESS_KEY }}
          R2_BUCKET: ${{ vars.R2_BUCKET }}
        run: mise run storage:r2-smoke
```

The workflow uses a `concurrency` group so two nightly runs cannot overlap.

## 5. Files

| File | Change |
| --- | --- |
| `.github/workflows/storage-r2-smoke.yaml` | New smoke workflow. |
| `.github/workflows/platform-images.yaml` | Use the plan login outputs. |
| `.github/workflows/platform-vms.yaml` | `disk-archive: auto`; the publish job. |
| `mise.toml` | Pin `aws-cli` and `zstd`. |
| `mise.lock` | Regenerate when the repository uses one. |
| `ci/platform-images/README.md` | Storage section, setup steps, costs, licences. |
| `ci/ewe-platform-image/README.md` | `[storage]` field documentation. |

## 6. Tasks

- [ ] Add the smoke workflow.
- [ ] Wire the plan login outputs into `platform-images.yaml`.
- [ ] Add `disk-archive: auto` and the publish job to `platform-vms.yaml`.
- [ ] Pin `aws-cli` and `zstd` in `mise.toml`; update the lock file.
- [ ] Write the README sections.
- [ ] Add the secrets table and the token policy to the README.
- [ ] Check every command in the README against the code help.

## 7. Verification

```bash
# The workflows parse and the schema is valid.
python3 - <<'PY'
import yaml, pathlib
for p in pathlib.Path('.github/workflows').glob('*.y*ml'):
    yaml.safe_load(p.read_text())
print("workflows parse")
PY

# The tools resolve from the pins.
mise which aws
mise which zstd

# The documented commands match the help.
nu ci/platform-images/scripts/vm.nu disk --help
nu ci/platform-images/scripts/r2.nu --help 2>/dev/null || true
```

The full check is the PR's GitHub CI, plus one manual run of the smoke workflow with secrets.

## 8. Done when

- Every workflow parses and uses the plan values.
- The smoke workflow skips on a fork and runs on the main repository.
- The documentation names every secret, its scope and its rotation.
- The cost table has the date of the prices.
- `progress.md` and `learnings.md` record the feature.

## 9. Risks

| Risk | Control |
| --- | --- |
| A literal registry or bucket name stays in one workflow. | `grep` for the old names in the PR review. |
| The smoke workflow runs on a fork and fails. | The secrets check exits 0 on a fork. |
| An old mise lock breaks the image build. | Regenerate the lock in the same change; the PR CI builds the image. |
| The README drifts from the code. | A task checks every command against the help output. |
