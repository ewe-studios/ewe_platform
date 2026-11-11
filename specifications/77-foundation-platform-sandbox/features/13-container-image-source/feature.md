---
feature: "Container image source"
status: "pending"
priority: "medium"
model: "sonnet"
phase: "3"
depends_on: ["12-sandbox-images", "10-docker-driver"]
crates: ["foundation_platform_sandbox"]
---

# Feature 13: A Docker image becomes a sandbox image

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); virtual machine
(VM); Secure Hash Algorithm 256 (SHA-256); behaviour-driven development (BDD); continuous
integration (CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner
of the STE standard ASD-STE100.

## 1. Goal

Turn a container image that the local Docker daemon has (or can pull) into a sandbox image
for the microVM drivers. A user writes a normal Dockerfile, builds it with Docker, and
runs the result in a Firecracker or libkrun sandbox:

```
agent-sandbox image import docker:python:3.12-slim --tag python
agent-sandbox run --image python -- python3 -c "print(1)"
```

This is the fast way to real images. It needs a Docker daemon at import time, not at run
time.

## 2. Story

- **S1**: "A user imports a Docker image and gets a sandbox image with the same files, file
  owners, start command, environment, work directory and user."
- **S2**: "The same import a second time does no work."
- **S3**: "The image is for another architecture, does not exist, or is larger than the
  image size."
- **S4**: "A user imports a root file system tar file with no Docker at all."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Import from Docker | user, tool | `ImageImporter::import(ImageSource::Docker(name), opts)` | An `ImageRef`. |
| U2 | Import a tar | user, tool | `ImageSource::Tar(path)` | An `ImageRef`. |
| U3 | Import the Alpine base | tool | `ImageSource::Alpine(version)` | An `ImageRef` (used by feature 14). |

## 3. Bounded context

**Inside this feature:** reading a container image's flat file tree and configuration,
mapping the configuration to a manifest, driving the fill of feature 12, the import cache.

**Outside:** the Docker client; `ImageStore`, `Writable` mode and unpack (feature 12);
layers and registries (feature 15).

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Image source | `Docker(name)`, `Tar(path)`, `Alpine(version)`. | the caller |
| Source identity | The text that names the exact content: the Docker image ID, the SHA-256 of a tar file, the artifact hash. | `ImageImporter` |
| Export stream | The tar stream of a created (not started) container. | the Docker daemon |
| Temporary container | A container made only for the export. Label `ewe.sandbox.import`. | `ImageImporter` |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| identify | entity | import | Finds the source identity and the image key. |
| export | entity | import | Opens the tar stream. |
| fill | story (feature 12) | import | Blank image, writable sandbox, unpack, stop. |
| map config | entity | import | Container configuration to `ImageManifest`. |
| import | story | caller | identify, cache check, export, fill, commit; undo on a failed step. |

## 4. Rules

### 4.1 Law worksheet

```
Entity: Import (story)
Conserved value:         the file tree in the sandbox image = the file tree of the container image (paths, types, modes, owners, link targets, content), minus the entries of section 6.4
Bound:                   unpacked bytes <= max_unpack_bytes and <= the image size
Inverse pairs:           none
Idempotent verbs:        import of a source identity that is in the store (returns the existing image; no export, no VM)
Ordering pairs:          identify -> export -> fill -> commit
Monotone values:         progress
Freeze points:           the image key = SHA-256("ewe-image-v1\n" + source identity + "\n" + arch + "\n" + size_mib), first 32 hex
Links to other entities: manifest.source = the source identity; manifest fields = the container configuration (section 6.3)

Entity: Temporary container
Conserved value:         0 containers with the import label after an import ends, good or bad
Bound:                   none
Inverse pairs:           create / remove
Idempotent verbs:        remove
Ordering pairs:          create -> export -> remove
Monotone values:         none
Freeze points:           none
Links to other entities: never started
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | The export stream is piped to the guest in chunks. The image is never held whole in memory or unpacked on the host. |
| I2 | File owners, modes, links and device nodes in the sandbox image equal those of the container image. |
| I3 | A second import of the same source identity, architecture and size returns the same key and starts nothing. |
| I4 | The temporary container is never started, and is removed on every path. |
| I5 | A failed import leaves no image in the store, no work file and no temporary container. |
| I6 | An image whose architecture is not the guest architecture is refused before the export. |
| I7 | The manifest gets `cmd`, `env`, `workdir` and `user` by the table of section 6.3, and nothing else from the configuration. |

### 4.3 Must have

- `ImageImporter`, `ImageSource`, `ImportOptions`, `ImportProgress`.
- `agent-sandbox image import | list | rm` and `agent-sandbox run --image`.

### 4.4 Must do

- Say in the result which configuration fields were dropped (section 6.3).

### 4.5 Must not

- Must not run a program of the image during the import.
- Must not need a Docker daemon to run an imported image.

## 5. Acceptance scenarios

```gherkin
Feature: Container image source

  Scenario: A Docker image runs as a microVM sandbox
    Given a Docker daemon with the image "python:3.12-slim" and a usable Firecracker host
    When the user imports "docker:python:3.12-slim" with the tag "python"
    And creates a sandbox with RootFs::Image("python") and runs ["python3", "-c", "print(1)"]
    Then stdout is "1\n"

  Scenario: Owners and modes survive
    Given a Docker image built with a file /data/f owned by 1000:1000 with mode 0640 and a setuid program
    When it is imported and a sandbox runs "stat" on both
    Then the owner, group and mode equal the Docker image's

  Scenario: The configuration becomes the manifest
    Given a Docker image with ENTRYPOINT ["/app"], CMD ["--serve"], ENV A=1, WORKDIR /srv, USER 1000
    When it is imported
    Then the manifest has cmd ["/app", "--serve"], env with A=1, workdir /srv and user "1000"
    And the report lists the dropped fields ExposedPorts and Volumes when the image has them

  Scenario: A second import does no work
    Given an imported image
    When the same image is imported again
    Then the result is the same key
    And no container was created and no sandbox was started

  Scenario: Another architecture is refused
    Given a Docker image for arm64 on an x86_64 host
    When it is imported
    Then the result is ImageArchMismatch
    And no container was created

  Scenario: An image that is too large fails cleanly
    Given a Docker image of 600 MiB and an import with size_mib 256
    When it is imported
    Then the result is an error that names the image size limit
    And the store has no new image, no work file is left, and no import container is left

  Scenario: A tar file is a source
    Given a root file system tar file
    When the user imports "tar:<path>" with cmd ["/bin/sh"]
    Then a sandbox from it runs ["/bin/sh", "-c", "echo ok"] with stdout "ok\n"

  Scenario: The Docker daemon is not needed at run time
    Given an imported image
    When the Docker daemon is stopped
    Then a sandbox from the image still starts and runs a command
```

## 6. Technical breakdown

### 6.1 API

`src/native/images/import.rs`, feature `images` (with `docker` for the Docker source):

```rust
pub enum ImageSource {
    Docker(String),                 // an image name or ID in the local daemon; pulled when missing
    Tar(PathBuf),                   // an uncompressed or gzip tar of a root file system
    Alpine(String),                 // a version of feature 03's manifest, for example "3.22"
}
pub struct ImportOptions {
    pub tag: Option<String>,
    pub size_mib: u32,              // default ImageLimits.default_size_mib (4096)
    pub cmd: Option<Vec<String>>,   // overrides the source's command; needed for Tar when a start command is wanted
    pub env: Vec<(String, String)>, // added over the source's
    pub pull: bool,                 // default true: pull a Docker image that the daemon does not have
}
pub enum ImportProgress { Identifying, Pulling, CacheHit, Filling { bytes: u64 }, Committing }

impl ImageImporter {
    pub fn new(ctx: Ctx, host: SandboxHost, store: ImageStore, control: Arc<dyn SandboxControl>,
               docker: Option<Arc<dyn DockerApi>>) -> Self;
    pub async fn import(&self, source: ImageSource, options: ImportOptions) -> SandboxResult<ImportReport>;
    pub fn import_with_progress(&self, source: ImageSource, options: ImportOptions) -> ImportStream;
}
pub struct ImportReport { pub image: ImageRef, pub manifest: ImageManifest, pub dropped_config: Vec<String>, pub cached: bool }
```

`control` is the control plane that starts the fill sandbox; its default driver must
support `Writable` (Firecracker, or libkrun with block support).

### 6.2 The import story

| Step | Work |
|---|---|
| 1. Identify | `Docker`: `image_inspect(name)` (404 and `pull`: `image_pull`, then inspect again). Identity: `docker:<name>@<image Id>`. Check `Architecture` against the guest architecture (`amd64` = x86_64, `arm64` = aarch64): `ImageArchMismatch`. `Tar`: identity `tar:sha256:<hash of the file>` (streamed). `Alpine`: identity `alpine:<artifact hash>`. |
| 2. Cache | Compute the key. If the store has it: set the tag when one is given; return with `cached: true`. |
| 3. Open the stream | `Docker`: `create_container` (name `ewe-import-<random>`, label `ewe.sandbox.import=1`, no start), then `container_export(id)`. `Tar`: the file, through `flate2` when its first two bytes are `1f 8b`. `Alpine`: the archive of feature 03, through `flate2`. |
| 4. Fill | `store.create_blank(size_mib)`; create a sandbox with `RootFs::Ext4Image(work)`, `RootMode::Writable`, `NetworkMode::None`; `sandbox.unpack("/", stream, UnpackOptions { oci_whiteouts: false })`; then the clean-up of section 6.4 through `exec`-free means (section 6.4); `sandbox.stop()`. |
| 5. Commit | Build the manifest (section 6.3); `store.commit(work, manifest, tag)`. |
| 6. Always | Remove the temporary container (`remove_container(id, force)`). |

Fault table:

| Step that fails | What the caller sees | What is left behind |
|---|---|---|
| Identify | `SubstrateApi` (Docker), `Io` (tar), `ImageArchMismatch` | nothing |
| Create container or export | `SubstrateApi` | nothing (the container is removed) |
| `create_blank` | `BinaryNotFound` or `Io` | nothing |
| The fill sandbox does not start | the start error | nothing (the work file is removed) |
| Unpack fails (`ENOSPC`, limits, bad tar) | `ArtifactTooLarge` with the message "the image is larger than size_mib = N; pass a larger size", or `SlaveError` with the entry path | nothing |
| `stop` reports `killed: true` | `Io { op: "unmount image" }`: the file may not be clean | nothing |
| Commit | `Io` | nothing |

`ENOSPC` from the unpack is mapped to `ArtifactTooLarge`.

### 6.3 Configuration to manifest

From `image_inspect` (`Config`):

| Container field | Manifest |
|---|---|
| `Entrypoint` + `Cmd` | `cmd` = the two lists joined; `None` when both are empty. `options.cmd` replaces it. |
| `Env` | `env`, each `K=V` split at the first `=`; then `options.env`. |
| `WorkingDir` | `workdir`; `None` when empty. |
| `User` | `user`; `None` when empty. |
| `Architecture` | `arch` |
| `ExposedPorts`, `Volumes`, `Healthcheck`, `StopSignal`, `Labels`, `OnBuild`, `Shell` | Dropped. Each one that is set goes to `ImportReport.dropped_config`. A sandbox has no port publishing and no volume in this spec. |

`Tar` and `Alpine`: `cmd` from the options, `env` from the options, no `workdir`, no `user`.

### 6.4 What a container export has that a root file system does not need

`docker export` gives the container's root. Docker adds a few files to it:

| Path | Action |
|---|---|
| `/.dockerenv` | Not unpacked. |
| `/etc/hosts`, `/etc/hostname`, `/etc/resolv.conf` | Kept as they are (empty files that Docker mounts over). The slave binds its own `resolv.conf` when the sandbox has a network. |
| `/dev/*` entries | Kept. The slave mounts `devtmpfs` over `/dev`. |

"Not unpacked" needs no command in the guest: `UnpackBegin` gets a field
`skip: Vec<String>` (exact paths, relative to `dest`); the tar reader drops those entries.
Add it to the messages of feature 12.

### 6.5 Command line

`cli::sandbox` gets:

```
agent-sandbox image import <docker:NAME | tar:PATH | alpine:VERSION> [--tag T] [--size-mib N] [--cmd JSON]
agent-sandbox image list
agent-sandbox image rm <tag-or-key>
agent-sandbox run --image <tag-or-key> [...] [-- <program> ...]     # with no program: the image's cmd
```

`image` sub-commands work on the local store (they are not in `SandboxService`); `run
--image` resolves locally and sends `RootFs::Image`.

## 7. Tests

| Law or scenario | Test | Where | Oracle |
|---|---|---|---|
| I7, "The configuration becomes the manifest" | `config_to_manifest` (each row of section 6.3; empty values; a `K=V=W` value) | `tests/images_import_*.rs` | example |
| key | `key_is_stable` (same inputs, same key; a change of each input changes it) | the same | identity |
| I3, I4, I5, I6, the fault table | `import_sim_ok`, `import_cache_hit`, `import_fault_matrix`, `import_arch_mismatch` (fake `DockerApi`, simulated control plane with a `ScriptedSlave` that accepts unpack) | the same | fault, expected outcome |
| I1 | `import_streams` (a 200 MiB fake export; the importer's peak buffer is under 1 MiB by a counting stream) | the same | bound |
| `skip` | `unpack_skip` | `tests/slave_unpack_*.rs` | expected outcome |
| I2, the first two scenarios | `e2e_import_python`, `e2e_import_owners` (builds a small image with `image_build`) | `tests/e2e_images.rs` (Docker and Firecracker; returns with the probe reports when not usable) | expected outcome |
| "An image that is too large ..." | `e2e_import_too_large` | the same | fault |
| "A tar file is a source" | `e2e_import_tar` (the Alpine archive) | the same | example |
| "The Docker daemon is not needed at run time" | covered by `e2e_import_tar` and by running `e2e_import_python`'s sandbox through a manager with no Docker driver | the same | example |
| CLI | `cli_image_parse`, `cli_image_flow` | `tests/cli_parse.rs`, `foundation_deployment_platform/tests/agent_sandbox_cli.rs` | example |

## 8. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --features "images docker testing slave" --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --features "images docker testing slave" --profile uat images_import slave_unpack cli_parse
cargo test -p foundation_platform_sandbox --features "images docker testing slave" --profile uat e2e_images -- --nocapture
```

## 9. Done when

- Each scenario of section 5 and each row of section 7 passes.
- `agent-sandbox image import docker:python:3.12-slim --tag python` followed by
  `agent-sandbox run --image python -- python3 -c "print(1)"` prints `1` on a Linux host
  with KVM and Docker.
- No import leaves a container, a work file or a partial image.
