---
feature: "One image for the VM volume and the installed software"
status: "planned"
priority: "medium"
depends_on: ["01-storage-config-and-manifest", "03-registry-backend"]
crates: []
issues: []
model: "sonnet"
branch: "spec74/f04-vm-volume-image"
---

# Feature 04: One image for the VM volume and the installed software

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** continuous integration (CI); GitHub Actions (GHA); GitHub
Container Registry (GHCR); Open Container Initiative (OCI); SHA-256; virtual machine (VM);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## 1. Goal

Remove the repeated setup work from a fresh-disk CI run. Build **one image** per OS and base.
The image holds the volume payload and the installed software. A job boots a fresh OS and
pulls the image to seed the volume. The image holds no vendor operating system files.

Owner decision (2026-11-23): do not copy a vendor OS disk. Use one image for the volume and
the installed software.

## 2. Measured state

1. `ci/platform-images/scripts/vm.nu:72` `main up` takes `--storage <dir>` and mounts it as
   `/storage` in the dockurr container. A fresh directory means a full OS install.
2. `ci/platform-images/actions/boot-vm/action.yml` passes `storage` through. For a
   GitHub-hosted runner the default is `/mnt/platform-images-vm`; that disk is lost after the
   job.
3. dockurr downloads the Windows or macOS installer from the vendor on the first boot. The
   vendor bits are not in the repository and are not in our images.
4. `.github/workflows/platform-vms.yaml` runs Windows only on the nightly schedule or a
   manual dispatch. macOS needs an Apple-hardware host with a manual install.
5. There is no command that publishes or fetches the volume state today.

## 3. Requirements

1. New commands in `vm.nu`:
   - `main volume build --os <windows|macos> --base-hash <hash> --source <dir> --config <path>`
   - `main volume push --os <os> --base-hash <hash> --source <dir> --config <path>`
   - `main volume prune --os <os> --config <path>`
2. The image holds the payload of `--source` at `/volume` and nothing else. It starts from
   `scratch`. It holds no vendor operating system files.
3. The image carries these labels:
   - `org.ewe.volume.os`: `windows` or `macos`.
   - `org.ewe.volume.base-hash`: the base image hash that produced the payload.
   - `org.ewe.volume.created`: RFC 3339 time.
   - `org.ewe.volume.sha256`: the SHA-256 of the payload archive, so a pull can verify it.
4. The image reference is `<registry>/<namespace>/ci-volume-<os>:<base-hash>`. The registry
   and namespace come from the `[storage]` config that feature 01 defines. Docker Hub is the
   default; GHCR is selectable.
5. `push` skips the upload when the tag exists with the same `org.ewe.volume.sha256`.
6. `push` stops the VM first. The container must not run. A running container is a typed
   error.
7. `pull` in the action uses the tag for the image's base hash. It extracts `/volume` into
   the storage directory. It verifies the SHA-256 label against the extracted archive.
8. The action gains a `volume-image` input. `auto` uses the tag for the current base hash.
   An empty value means a fresh start. An explicit reference is used as given.
9. The action refuses a volume image whose `org.ewe.volume.os` does not match the job's OS.
10. No credential reaches the guest. The pull happens on the host.
11. No secret is needed for a public Docker Hub image. GHCR uses `GITHUB_TOKEN`.
12. A full guest disk, only where the licence allows, exports to the selected bulk store:
    `main disk export --os <os> --base-hash <hash> --source <dir> --format
    <tar.zst|qcow2|ova|vhdx|raw> --config <path>`. The client writes
    `<sha256>.<format>`. This path is off by default; the volume image is the normal path.

## 4. Design

### 4.1 Image build

`volume build` writes a temporary Dockerfile and builds a data-only image:

```dockerfile
FROM scratch
COPY volume/ /volume/
```

It then adds the labels above with `--label`. The payload directory is the volume state that
the job needs: the installed software, its configuration and its data. It is not the vendor
OS disk.

### 4.2 Push

`push` runs `docker build` and `docker push` for the tag. Before the build it computes the
SHA-256 of the payload archive and writes it to the label. When `docker manifest inspect`
shows the tag and its label already match, the command prints `unchanged` and exits 0.

The registry is selected by `[storage] backend`: `dockerhub` (default) or `ghcr`. The push
uses the same credentials as the existing image workflow: `DOCKERHUB_EWESTUDIO_TOKEN` for
Docker Hub, `GITHUB_TOKEN` for GHCR.

### 4.3 Pull in `boot-vm`

```yaml
inputs:
  volume-image:
    description: "auto (tag for the base hash), an explicit image reference, or empty for a fresh start."
    default: "auto"
```

The step:

1. Parse the base hash from the image tag (the part after `hash-`).
2. Run `nu ci/platform-images/scripts/vm.nu volume pull --os "$OS" --base-hash "$BASE_HASH"
   --dest "$STORAGE" --config "$CONFIG"`.
3. Print the image reference, the bytes and the age in the step summary.

### 4.4 Workflows

`platform-vms.yaml`:

1. `windows-ssh` gains `volume-image: auto`. An empty store falls back to a fresh start and
   prints a warning.
2. A new nightly job `windows-publish-volume` runs `windows-ssh` with a fresh storage path and
   then `vm.nu volume push`. It does not push when the base hash and the payload hash are
   unchanged.
3. `macos-ssh` gains the same value. On the Apple host the persistent storage is already the
   cache; the pull is a no-op after the check.

The push job runs only on the nightly schedule in the main repository. A fork pull request
has no push credential.

### 4.5 Licence

The image holds our volume payload and our installed software. It holds no vendor operating
system files; dockurr downloads those from the vendor on first boot. The image stays private
until the owner decides otherwise. A public image would expose our configuration and data.

### 4.6 Idempotence

A nightly publish with an unchanged base image and payload finds the same tag and label. It
does not build or push. The job prints `unchanged` and exits 0.

## 5. Files

| File | Change |
| --- | --- |
| `ci/platform-images/scripts/vm.nu` | Add `volume build`, `volume push`, `volume pull`, `volume prune`. |
| `ci/platform-images/actions/boot-vm/action.yml` | Add `volume-image`; call `volume pull`; print the summary. |
| `.github/workflows/platform-vms.yaml` | Pass `volume-image`; add the nightly publish job. |
| `ci/platform-images/README.md` | Document the image, the labels, the cache and the commands. |
| `ci/platform-images/tests/vm_volume_tests.nu` | New tests. |
| `ci/platform-images/tests/docker-compose.yml` | Add a local registry (`registry:2`) for the tests. |

## 6. Tasks

- [ ] Write `volume build` with the scratch image, the `/volume` payload and the labels.
- [ ] Write `volume push` with the stopped-VM check, the unchanged check and the registry
      selection.
- [ ] Write `volume pull` with the tag, the extraction and the SHA-256 verification.
- [ ] Write `volume prune` with `retain_versions = 1`.
- [ ] Add `volume-image` to `boot-vm` and the summary output.
- [ ] Update `platform-vms.yaml` and add the nightly publish job.
- [ ] Update the README.
- [ ] Add the tests in section 7.
- [ ] Run one full real-registry round trip and record the size and the time in
      `progress.md`.

## 7. Tests

| Test | Asserts |
| --- | --- |
| `test volume push refuses a running VM` | A typed error names the container. |
| `test volume build holds only the payload` | The image has `/volume` and no OS files. |
| `test volume push round trip` | A 64 MiB payload uploads; the tag has the labels and the size. |
| `test volume pull verifies sha` | A changed payload is rejected and the destination stays empty. |
| `test volume pull rejects the wrong OS` | A Windows image on a macOS job is a typed error. |
| `test volume pull reuses the cache` | A second pull makes no download. |
| `test volume push idempotent` | A second push of the same content prints `unchanged` and pushes nothing. |
| `test volume prune keeps one` | Two base hashes leave the newest image only. |
| `test registry backend switches` | `ghcr` changes the image reference; `dockerhub` is the default. |

## 8. Verification

```bash
docker compose -f ci/platform-images/tests/docker-compose.yml up -d registry minio
mise run test:ci-storage -- --case volume
```

Real registry, manual:

```bash
mise run storage:registry-smoke -- --case volume
```

## 9. Done when

- A fresh GitHub-hosted Windows run boots from the volume image, without the repeat setup
  work.
- The pull verifies the SHA-256; a corrupted payload cannot seed the volume.
- The image has no vendor operating system files.
- The nightly job does not push an unchanged payload.
- `progress.md` and `learnings.md` record the feature, the measured size and the duration.

## 10. Risks

| Risk | Control |
| --- | --- |
| A live volume is archived and is corrupt. | The stopped-container check and a sync before the build. |
| The image is large. | It holds the payload only; the vendor OS is not included. The nightly push has time. |
| A stale payload hides a provisioning regression. | The base hash is part of the tag; a new image gets a fresh start. |
| A public image exposes our configuration. | The image stays private; the owner decides any public copy. |
| The vendor installer changes on first boot. | The payload holds our software only; a vendor download is expected. |
