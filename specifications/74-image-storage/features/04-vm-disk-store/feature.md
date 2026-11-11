---
feature: "Publish, fetch and prune provisioned VM disks in R2"
status: "planned"
priority: "medium"
depends_on: ["01-storage-config-and-manifest", "02-r2-object-client"]
crates: []
issues: []
model: "sonnet"
branch: "spec74/f04-vm-disk-store"
---

# Feature 04: VM disk store

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Cloudflare R2 (R2); continuous integration (CI); end-user
licence agreement (EULA); gigabyte (GB); gibibyte (GiB); GitHub Actions (GHA); Secure Shell
(SSH); SHA-256; virtual machine (VM); AeroSpace and Defence Industries Association of Europe
(ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Remove the 45–75 minute Windows install from a fresh-disk CI run. A job downloads a verified,
compressed provisioned disk from R2, extracts it and boots the VM. A nightly job provisions a
new disk and publishes it. The macOS manual install becomes recoverable from the archive.

## 2. Measured state

1. `ci/platform-images/scripts/vm.nu:72` `main up` takes `--storage <dir>` and mounts it as
   `/storage` in the dockurr container. A fresh directory means a full OS install.
2. `ci/platform-images/actions/boot-vm/action.yml` passes `storage` through. For a
   GitHub-hosted runner the default is `/mnt/platform-images-vm`; that disk is lost after the
   job.
3. `ci/platform-images/README.md` states that a provisioned disk is 40–50 GB raw and 15–20 GB
   compressed, that a public copy violates the licence, and that a persistent self-hosted
   `/storage` is the current answer.
4. `.github/workflows/platform-vms.yaml` runs Windows only on the nightly schedule or a
   manual dispatch. macOS needs an Apple-hardware host with a manual install.
5. There is no upload, download or prune command.

## 3. Requirements

1. New commands in `vm.nu`:
   - `main disk push --os <windows|macos> --base-hash <hash> --storage <dir> --config <path>`
   - `main disk pull --os <os> --base-hash <hash> --dest <dir> --config <path>`
   - `main disk list --os <os> --config <path>`
   - `main disk prune --os <os> --base-hash <hash> --config <path>`
2. `push` stops the VM first. The container must not run. The command checks that the named
   container is stopped before it reads the disk. A running container is a typed error.
3. `push` compresses the storage directory with `tar` and `zstd`, computes the SHA-256,
   uploads the body and the checksum file, and writes the manifest last.
4. `push` refuses a licence value that does not match the OS: `internal-evaluation` for
   Windows, `internal-apple-hardware` for macOS. The value comes from the command, not from
   the caller's environment.
5. `push` skips the upload when the same SHA-256 already exists. It still updates the
   manifest only if the base hash or the base image changed.
6. `pull` reads the newest manifest for the OS and base hash, verifies the size and the
   SHA-256, and extracts to the destination. It keeps a local cache at
   `<dest>/.cache/<sha256>/` and reuses it when the hash matches.
7. `pull` refuses a manifest with a different `base_hash`. A job must not boot a disk from a
   different base image.
8. `prune` keeps the newest `retain_versions` manifests for the OS and base hash and deletes
   the older bodies, checksum files and manifests. It prints each key.
9. `boot-vm` gains a `disk-archive` input. When it is not empty, the action runs
   `vm.nu disk pull` before `docker run`. When `storage` is empty, the action uses the default
   path as today.
10. The action refuses `disk-archive` for macOS unless `host-runs-on` names
    `apple-hardware`. The action cannot see the host label; the caller workflow enforces the
    rule. The action documents the rule and prints it in the step summary.
11. No credential reaches the guest. The download happens on the host.

## 4. Design

### 4.1 Compression format

`tar --zstd` writes one file. Use the repository's `tar` and `zstd` (feature 02 pins zstd).
Command shape:

```bash
tar --create --file - --directory "$storage" . | zstd -T0 -3 -o "$archive"
```

`zstd -3` is the default level and is the best trade between CPU and size for a disk image.
The level is a config field (`[storage.r2] compression_level`, default 3, source: zstd
default).

The archive holds the whole storage directory, including the dockurr disk file and its
metadata. The destination must be empty or already match the cache.

### 4.2 Disk pull in `boot-vm`

```yaml
inputs:
  disk-archive:
    description: "R2 key of the disk archive, e.g. disks/windows/<base-hash>/manifest.json. Empty means a fresh install."
    default: ""
```

The step:

1. Validate the OS and the archive prefix.
2. Run `nu ci/platform-images/scripts/vm.nu disk pull --os "$OS" --base-hash "$BASE_HASH"
   --dest "$STORAGE" --config "$CONFIG" --key "$DISK_ARCHIVE"`.
3. Print the manifest summary in the step summary: version, bytes, age.

The `base-hash` comes from the image ref: the tag after `hash-`. The action parses it from
`image`.

### 4.3 Workflows

`platform-vms.yaml`:

1. `windows-ssh` gains `disk-archive: auto` for the nightly schedule. `auto` means the action
   lists the newest manifest for the image's base hash and uses it when it exists. An empty
   store falls back to a fresh install and prints a warning.
2. A new nightly job `windows-publish-disk` runs `windows-ssh` with a fresh storage path and
   then `vm.nu disk push`. It does not run on every schedule when the base hash is unchanged.
3. `macos-ssh` gains the same `auto` value. On the Apple host the persistent storage is
   already the cache; the pull is a no-op after the cache check.

The workflow has no R2 write credential in a fork PR. The push job runs only on the nightly
schedule in the main repository.

### 4.4 Licence

The manifest records the licence. The uploader sets it. The README states:

- Windows: internal evaluation copies only. No redistribution. Delete on request.
- macOS: Apple hardware only. The archive never leaves the private bucket.

### 4.5 Idempotence

A nightly publish with no image change finds the same SHA-256. It does not upload. The
manifest stays. The job prints `unchanged` and exits 0.

## 5. Files

| File | Change |
| --- | --- |
| `ci/platform-images/scripts/vm.nu` | Add `disk push`, `disk pull`, `disk list`, `disk prune`. |
| `ci/platform-images/actions/boot-vm/action.yml` | Add `disk-archive`; call `disk pull`; print the summary. |
| `.github/workflows/platform-vms.yaml` | Pass `disk-archive`; add the nightly publish job. |
| `ci/platform-images/README.md` | Document the archive, the cache, the licence and the commands. |
| `ci/platform-images/tests/vm_disk_tests.nu` | New tests. |
| `ci/platform-images/tests/docker-compose.yml` | Reuse MinIO; no new service. |

## 6. Tasks

- [ ] Write `disk push` with the stopped-container check, the archive, the SHA-256 and the
      licence guard.
- [ ] Write `disk pull` with the manifest, the checksum, the extraction and the local cache.
- [ ] Write `disk list` and `disk prune`.
- [ ] Add `disk-archive` to `boot-vm` and the summary output.
- [ ] Update `platform-vms.yaml` and add the nightly publish job.
- [ ] Update the README.
- [ ] Add the tests in section 7.
- [ ] Run one full real-bucket round trip and record the sizes and the times in
      `progress.md`.

## 7. Tests

| Test | Asserts |
| --- | --- |
| `test disk push refuses a running VM` | A typed error names the container. |
| `test disk push refuses a wrong licence` | Windows with the macOS licence, and the reverse, fail. |
| `test disk push round trip` | A 64 MiB fixture directory uploads; the manifest has the size and the hash. |
| `test disk pull verifies sha` | A corrupted object is rejected and the destination stays empty. |
| `test disk pull rejects base hash` | A manifest from another base hash is a typed error. |
| `test disk pull reuses cache` | A second pull makes no download. |
| `test disk push idempotent` | A second push of the same content uploads no part. |
| `test disk prune keeps newest n` | Three versions with `retain_versions = 2` leave two. |

## 8. Verification

```bash
docker compose -f ci/platform-images/tests/docker-compose.yml up -d minio
mise run test:ci-storage -- --case disk
```

Real bucket, manual:

```bash
mise run storage:r2-smoke -- --case disk-1gib
```

## 9. Done when

- A fresh GitHub-hosted Windows run boots from the archive in about a minute after the
  download, without the OS install.
- The download verifies the SHA-256; a corrupted object cannot boot.
- The archive has no public access path.
- The nightly job does not upload an unchanged disk.
- `progress.md` and `learnings.md` record the feature, the measured size and the duration.

## 10. Risks

| Risk | Control |
| --- | --- |
| A live disk is archived and is corrupt. | The stopped-container check and a `sync` before `tar`. |
| The archive is huge and the upload takes longer than the job. | Upload once in the nightly job; the CI job only downloads. 64 MiB parts; resume. |
| A stale disk hides a provisioning regression. | The image base hash is part of the path; a new image gets a fresh install. |
| A license violation. | Private bucket; fixed license value; owner decision in `requirements.md` section 13. |
| The macOS disk is moved to non-Apple hardware. | Documentation and the workflow check; the archive is private; the owner decision. |
