---
feature: "Sandbox images"
status: "pending"
priority: "medium"
model: "sonnet"
phase: "3"
depends_on: ["06-programs-rpc-provider", "09-libkrun-driver"]
crates: ["foundation_platform_sandbox", "sandbox_krun"]
---

# Feature 12: Sandbox images (the image form, a writable root, and unpack)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** remote procedure call (RPC); virtual machine (VM); user
identifier (UID); group identifier (GID); Secure Hash Algorithm 256 (SHA-256); JavaScript
Object Notation (JSON); behaviour-driven development (BDD); continuous integration (CI);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## 1. Goal

Give the microVM drivers a common image: one ext4 file with correct file owners, and a
manifest with the start command. This feature adds the three parts that features 13 to 15
build on:

1. The image form and the image store.
2. A writable root mode, so a sandbox can change an image file on purpose.
3. A slave call that unpacks a tar stream into the root as the guest's root user.

The idea that removes the hard problem: a normal host user cannot make root-owned files in
an image. A guest can. So the host makes an empty ext4 file, boots a sandbox on it with a
writable root, and the slave fills it.

## 2. Story

- **S1**: "A tool makes an empty image and fills it from a tar stream; the files have the
  owners and modes of the tar."
- **S2**: "A caller boots a sandbox from an image by its name, and the image's start
  command, environment and work directory apply."
- **S3**: "The same image boots on Firecracker and on libkrun."
- **S4**: "A tar stream tries to write outside the target, or has a whiteout."
- **S5**: "A command runs as the user that the image names."
- **S6**: "The fill fails halfway."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Make an empty image | image tool | `ImageStore::create_blank(size_mib)` | A sparse ext4 file. |
| U2 | Boot writable | image tool | a spec with `RootMode::Writable` | Programs and unpack write to the image file. |
| U3 | Unpack | image tool | `Sandbox::unpack(dest, tar_stream, options)` | The tree is in the image, with owners. |
| U4 | Commit an image | image tool | `ImageStore::commit(work, manifest)` | A named, immutable image. |
| U5 | Boot an image | caller | `RootFs::Image(ref)` | A sandbox with the manifest defaults. |
| U6 | Run the start command | caller | `Sandbox::start_cmd()` | The manifest `cmd` runs. |

## 3. Bounded context

**Inside this feature:** `ImageManifest`, `ImageStore`, `RootMode::Writable`, the `Unpack`
calls, the `user` of a command, ext4 on libkrun.

**Outside:** where an image's content comes from (features 13 to 15); the drivers and the
slave, which this feature extends in small, named ways.

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Image | A directory `<cache_dir>/images/store/<key>/` with `manifest.json` and `rootfs.ext4`. Immutable after commit. | `ImageStore` |
| Image key | 32 hex characters: the first half of a SHA-256 over the image's source identity. | the tool that commits |
| Tag | A file `<cache_dir>/images/tags/<name>` with a key. | `ImageStore` |
| Work image | An ext4 file in `<cache_dir>/images/work/`, not yet an image. | the tool that made it |
| `ImageManifest` | Architecture, command, environment, work directory, user, kernel reference, source. | the tool that commits |
| Unpack session | One tar stream being applied in the guest. | the slave |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| create blank | entity | tool | `mkfs.ext4` on a new sparse file. |
| unpack | story | tool | Stream a tar into the guest; apply entries in order. |
| commit | entity | tool | Move a work image into the store; write the manifest; set a tag. |
| resolve | entity | `create` | `ImageRef` to the image directory. |
| snapshot | entity | tool | Copy a work image (reflink when the file system has it). |

## 4. Rules

### 4.1 Law worksheet

```
Entity: Image (in the store)
Conserved value:         the bytes of rootfs.ext4 and manifest.json never change after commit
Bound:                   none
Inverse pairs:           commit / remove
Idempotent verbs:        commit of a key that exists (keeps the first; removes the new work file); resolve
Ordering pairs:          create blank -> unpack -> commit
Monotone values:         none
Freeze points:           everything at commit
Links to other entities: a tag names exactly one key; manifest.arch = the architecture of the programs in rootfs.ext4

Entity: Unpack session
Conserved value:         after a good unpack: for every entry of the tar, the path under dest has the entry's type, mode, owner and content
Bound:                   entries <= max_unpack_entries; bytes <= max_unpack_bytes; no path leaves dest
Inverse pairs:           none
Idempotent verbs:        UnpackChunk at an applied offset (as FileWriteChunk)
Ordering pairs:          entries are applied in tar order; a later entry replaces an earlier one at the same path
Monotone values:         offset
Freeze points:           dest and options at begin
Links to other entities: the sandbox must be in RootMode::Writable (or unpack to a tmpfs path in other modes)

Entity: Writable root
Conserved value:         what programs write = what the image file holds after a clean stop
Bound:                   free space of the image
Inverse pairs:           none
Idempotent verbs:        none
Ordering pairs:          Shutdown -> unmount -> power off (so the file system is clean)
Monotone values:         none
Freeze points:           none
Links to other entities: only one sandbox at a time uses a work image in Writable mode
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | A committed image never changes. A sandbox on `RootFs::Image` attaches it read-only (feature 05, invariant I2). |
| I2 | After an unpack of a tar, each path has the type, mode, UID, GID, link target and content of its last tar entry. |
| I3 | No tar entry writes outside `dest`: the unpack runs in a child process whose root is `dest`. |
| I4 | An unpack stops at `max_unpack_entries` or `max_unpack_bytes` with `ArtifactTooLarge`. |
| I5 | A failed unpack reports the entry path and the `errno`. The work image is then not usable; the tool removes it. |
| I6 | In `Writable` mode, a clean `Shutdown` leaves an ext4 file that mounts with no journal replay needed. |
| I7 | A work image is used by one `Writable` sandbox at a time: the driver takes an exclusive lock on the file and fails with `InvalidSpec { field: "rootfs" }` when it is held. |
| I8 | `RootFs::Image` gives the same program behaviour on Firecracker and on libkrun (the shared acceptance scenarios pass on both). |
| I9 | The manifest defaults apply only where the spec has its own default: an explicit spec value wins. |
| I10 | A command with `user` runs with that UID and GID and no extra group; without it, as root. |
| I11 | An image whose `arch` is not the driver's guest architecture is refused with `ImageArchMismatch` before a start. |

### 4.3 Must have

- `ImageManifest`, `ImageRef`, `ImageStore`.
- `RootMode::Writable` in the contract, the slave and both microVM drivers.
- `UnpackBegin`, `UnpackChunk`, `UnpackCommit`, `UnpackAbort` in `SlaveService` and
  `SandboxService`; `Sandbox::unpack`.
- `Command.user` and `ExecRequest.user`.
- `rootfs_ext4` on libkrun when the library has block support.

### 4.4 Must do

- Report `mkfs.ext4` in the host check.
- Unmount cleanly at `Shutdown` in `Writable` mode.

### 4.5 Must not

- Must not fill an image on the host with `mkfs.ext4 -d`: it copies the host owner of each
  file.
- Must not open an image as any format but raw (the libkrun header warns that format
  probing is a security risk).

## 5. Acceptance scenarios

```gherkin
Feature: Sandbox images

  Scenario: The guest fills an image with the right owners
    Given a blank work image of 256 MiB
    And a tar with /bin/tool (mode 4755, uid 0), /home/u/f (uid 1000, gid 1000) and a symlink /l -> /bin/tool
    When a Writable sandbox on the work image unpacks the tar to "/" and stops
    And a second sandbox boots the work image read-only
    Then "stat -c '%a %u %g' /bin/tool" prints "4755 0 0"
    And "stat -c '%u %g' /home/u/f" prints "1000 1000"
    And "readlink /l" prints "/bin/tool"

  Scenario: A tar cannot write outside its target
    Given a tar with the entries "../x" and a symlink "d -> /" followed by "d/y"
    When it is unpacked to "/data"
    Then nothing exists outside /data that the tar named
    And /data/d/y is inside /data (the symlink target is resolved inside the target root)

  Scenario: Whiteouts remove earlier entries
    Given an image that has /a/b and /c/d
    When a tar with the entries "a/.wh.b" and "c/.wh..wh..opq" and "c/e" is unpacked with oci_whiteouts true
    Then /a/b does not exist, /c/d does not exist, and /c/e exists

  Scenario: An unpack that is too large stops
    Given max_unpack_bytes is 1 MiB
    When a tar of 10 MiB is unpacked
    Then the result is ArtifactTooLarge

  Scenario: An image boots by name with its defaults
    Given a committed image "tool" with cmd ["/bin/tool", "--serve"], env A=1 and workdir /srv
    When the caller creates a sandbox with RootFs::Image("tool") and calls start_cmd
    Then the program /bin/tool runs with the argument --serve, A=1 and the directory /srv

  Scenario: The spec wins over the manifest
    Given the same image
    When the caller creates a sandbox with workdir /tmp
    Then commands run in /tmp

  Scenario: The same image runs on both microVM drivers
    Given a committed image
    When the shared acceptance scenarios run on Firecracker and on libkrun with that image
    Then both give the same results

  Scenario: A committed image does not change
    Given the SHA-256 of an image's rootfs.ext4
    When a sandbox on it writes files and stops
    Then the SHA-256 is the same

  Scenario: A command runs as the image user
    Given an image with user "1000:1000"
    When the caller runs ["id", "-u"]
    Then stdout is "1000\n"

  Scenario: An image for another architecture is refused
    Given an image with arch aarch64 on an x86_64 host
    When the caller creates a sandbox from it
    Then the result is ImageArchMismatch

  Scenario: Two writers on one work image are refused
    Given a Writable sandbox on a work image
    When a second Writable sandbox is created on the same file
    Then the result is InvalidSpec { field: "rootfs" }
```

## 6. Technical breakdown

### 6.1 Facts

| Fact | Source |
|---|---|
| The Firecracker CI kernels and both `libkrunfw` kernels have `EXT4_FS`, `VIRTIO_BLK`, `OVERLAY_FS` and `TMPFS` built in. | `learnings.md` sections 2.5 and 3.8 |
| The `libkrunfw` x86_64 kernel has no squashfs. So ext4 is the one format that all microVM kernels of this spec can mount. | `config-libkrunfw_x86_64`: `# CONFIG_SQUASHFS is not set` |
| libkrun attaches a raw disk with `krun_add_disk2(ctx, block_id, path, KRUN_DISK_FORMAT_RAW, read_only)`. Block support is a build option (`BLK=1`), reported by `krun_has_feature(KRUN_FEATURE_BLK)`. | `include/libkrun.h` lines 177 to 226 and 1106 to 1132; `Makefile` line 39 |
| Homebrew builds libkrun with `BLK=1`. The Nix package has it off by default. | `homebrew-core` `Formula/lib/libkrun.rb` line 74; `slp/homebrew-krun` `Formula/libkrun.rb` line 25; `nixpkgs` `pkgs/by-name/li/libkrun/package.nix` line 19 |
| libkrun's virtio-fs keeps file owners only on macOS (an extended attribute); on Linux it shows the host owner. So a directory is not a portable image form. | `src/devices/src/virtio/fs/macos/passthrough.rs` line 37; `linux/passthrough.rs` |
| `mkfs.ext4 -E root_owner=0:0` sets the owner of the root directory. | e2fsprogs `mke2fs` manual |

### 6.2 The image form

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageManifest {
    pub schema: u32,                       // 1
    pub key: String,                       // 32 hex characters
    pub arch: GuestArch,
    pub created_unix_ms: u64,              // ctx.clock()
    pub cmd: Option<Vec<String>>,
    pub env: Vec<(String, String)>,
    pub workdir: Option<String>,
    pub user: Option<String>,              // "uid", "uid:gid", "name" or "name:group"
    pub kernel: Option<String>,            // a manifest name of feature 03, for Firecracker
    pub rootfs_sha256: String,
    pub rootfs_size: u64,
    pub source: String,                    // free text: "docker:alpine:3.22@sha256:...", "sandboxfile:<digest>"
    pub steps: Vec<String>,                // build file lines, for inspection; empty for a converted image
}
pub enum ImageRef { Tag(String), Key(String) }

impl ImageStore {
    pub fn new(ctx: Ctx, host: SandboxHost) -> Self;
    /// A new sparse ext4 file in images/work/. Runs mkfs.ext4.
    pub async fn create_blank(&self, size_mib: u32) -> SandboxResult<PathBuf>;
    /// A copy of an image or a work image, as a new work image.
    pub async fn snapshot(&self, source: &Path) -> SandboxResult<PathBuf>;
    pub async fn commit(&self, work: PathBuf, manifest: ImageManifest, tag: Option<&str>) -> SandboxResult<ImageRef>;
    pub async fn resolve(&self, image: &ImageRef) -> SandboxResult<(PathBuf, ImageManifest)>;   // (rootfs path, manifest)
    pub async fn list(&self) -> SandboxResult<Vec<ImageManifest>>;
    pub async fn remove(&self, image: &ImageRef) -> SandboxResult<()>;
}
```

- `create_blank`: `mkfs.ext4 -q -F -m 0 -E root_owner=0:0 <file> <size_mib>M`, through
  `host.processes`. The program path comes from `HostConfig.mkfs_ext4_path` (a new field):
  `EWE_MKFS_EXT4`; `mkfs.ext4` on `PATH`; `/sbin/mkfs.ext4`; `/usr/sbin/mkfs.ext4`;
  `/opt/homebrew/opt/e2fsprogs/sbin/mkfs.ext4`; `/usr/local/opt/e2fsprogs/sbin/mkfs.ext4`.
  Not found: `BinaryNotFound { name: "mkfs.ext4" }`; the message for macOS says
  `brew install e2fsprogs`. `mkfs.ext4` writes only metadata, so the file is sparse.
- `snapshot`: tries a reflink (`ioctl FICLONE` on Linux, `clonefile` on macOS); if the file
  system refuses, a copy that keeps holes (`SEEK_DATA` / `SEEK_HOLE`). Both are in
  `native/host.rs` as `HostFs::clone_file(from, to)`.
- `commit`: compute the SHA-256 and the size of the work file (streamed); if
  `store/<key>/` exists, remove the work file and return the existing reference; if not,
  make `store/<key>.tmp-<random>/`, move the file in as `rootfs.ext4`, write
  `manifest.json`, rename the directory, then write the tag file (temporary file and
  rename).
- `remove` of an image that a tag names removes the tag too.

Image size: the default `size_mib` is `ImageLimits.default_size_mib` = 4096. The file is
sparse, so its disk use is its content. A guest that fills it gets `ENOSPC`; the error
message names the limit.

### 6.3 Booting an image

`SandboxManager::create` with `RootFs::Image(r)`: before driver choice it resolves the
image (it holds an `Option<ImageStore>`; `None` gives `InvalidSpec { field: "rootfs" }`),
checks `manifest.arch` against `GuestArch::host()` (`ImageArchMismatch`), and rewrites the
spec: `rootfs = Ext4Image(path)`; `env` = manifest env then spec env; `workdir` = the spec
value when it is not the default `/workspace`, else the manifest value, else `/workspace`;
`kernel` from the manifest when the spec has `None`. The record keeps the manifest.

`Sandbox::start_cmd()`: `exec(Command { argv: manifest.cmd, user: manifest.user, .. })`;
with no `cmd`: `InvalidSpec { field: "cmd" }`. Commands of that sandbox get
`user: manifest.user` when the command has none.

### 6.4 `RootMode::Writable`

Contract (feature 00): a third `RootMode` value. `validate` allows it only when the driver
lists it in `root_modes` and the root is `Ext4Image`.

Slave (feature 01, section 6.5): for `BlockDevice` with `Writable`: step 1 mounts the
device read-write at `/run/ewe/root` (no lower layer, no overlay); step 3 makes missing
mount points; the rest is the same. `Shutdown` in this mode, after the programs end:
unmount the bind mounts, `sync()`, `umount("/run/ewe/root")`; if the unmount fails with
`EBUSY`, remount read-only (`MS_REMOUNT | MS_RDONLY`), which also makes the file system
clean; then power off or exit.

Drivers:

- Firecracker (feature 05): the drive has `is_read_only: false` for `Writable`.
  `root_modes` gets `Writable`.
- libkrun (feature 09): section 6.5.

Both take an exclusive advisory lock on the image file for a `Writable` sandbox
(`flock(LOCK_EX | LOCK_NB)` through `HostFs::lock_exclusive(path) -> SandboxResult<FileLock>`),
held by the substrate handle until `cleanup`.

### 6.5 ext4 on libkrun

Runner (`bin/sandbox_krun`): `KrunRunnerConfig` gets
`disks: Vec<KrunDisk { id: String, path: PathBuf, read_only: bool }>`. For each, after
call 6 of feature 09's sequence: `krun_add_disk2(ctx, id, path, 0 /* RAW */, read_only)`.
The symbol is resolved only when `disks` is not empty; missing: exit code 71. The runner's
`--probe` output gets `"blk": true|false` from `krun_has_feature(1)`.

Driver: for `RootFs::Ext4Image`, the boot root is the same private directory, there is no
`user_root`, the image is the disk `rootfs`, and the substrate's root is
`BlockDevice { device: "/dev/vda", fstype: Ext4 }`. `rootfs_ext4` is the probe's `blk`
value; `root_modes` gets `Writable` when it is true. With `blk` false, an image spec is
rejected with `UnsupportedRootFs`, and the probe check `blk` says: "build libkrun with
`BLK=1`".

### 6.6 Unpack

Four calls in both services, with the shape of the file write calls:

```rust
pub struct UnpackBeginRequest { pub dest: String, pub oci_whiteouts: bool }
pub struct UnpackBeginResponse { pub upload_id: String }
pub struct UnpackChunkRequest { pub upload_id: String, pub offset: u64, pub data: Vec<u8> }   // tar bytes, not compressed
pub struct UnpackChunkResponse { pub next_offset: u64 }
pub struct UnpackCommitRequest { pub upload_id: String, pub size: u64 }
pub struct UnpackCommitResponse { pub entries: u64, pub bytes: u64 }
pub struct UnpackAbortRequest { pub upload_id: String }

impl Sandbox {
    /// `tar` is an uncompressed tar byte stream.
    pub async fn unpack(&self, dest: &str, tar: impl ByteStream, options: UnpackOptions) -> SandboxResult<UnpackReport>;
}
```

In the slave (`native/slave/unpack.rs`):

- `UnpackBegin`: `dest` joined to `exec_root` (it must exist and be a directory). Fork a
  child: `chroot(<exec_root><dest>)`, `chdir("/")`, then read tar bytes from a pipe. The
  parent keeps the pipe's write end.
- `UnpackChunk`: `accept_span`; write the bytes to the pipe (it parks on pipe writability;
  the call returns after the write, so the caller's pace follows the child's).
- `UnpackCommit`: close the pipe; wait for the child; the child writes a result line
  (entries, bytes, or the failed entry path and `errno`) to a second pipe.
- The child applies entries in order. It supports ustar, PAX extended headers (`path`,
  `linkpath`, `size`, `uid`, `gid`, and `SCHILY.xattr.security.capability`) and GNU long
  names (`L`, `K`). Types: regular file, directory, symbolic link, hard link, character
  and block device, FIFO. For each: remove what is at the path when its type differs;
  create; `lchown(uid, gid)`; `chmod(mode)` (not for a symbolic link); set the
  modification time. A hard link target must be an earlier entry.
- A path with a `..` part or an absolute path is made relative by dropping leading `/`
  and rejecting `..` (`BadRequest`, the unpack fails). Symbolic links inside the tree
  resolve inside the child's root, because the child is in a `chroot`.
- `oci_whiteouts: true`: an entry whose file name is `.wh..wh..opq` removes every child of
  its directory that this unpack did not create (the child keeps the set of paths it
  created in this session); an entry `.wh.<name>` removes `<name>` in its directory, with
  everything below it. Whiteout entries create no file.
- Limits from `SlaveLimits`: `max_unpack_entries` (default 1 000 000) and
  `max_unpack_bytes` (default 16 GiB), both added to `SandboxLimits`. Passing one fails
  with `Io` and the text `unpack limit`; the control side maps it to `ArtifactTooLarge`.
- The tar reader is hand-written in the slave (about 300 lines): the slave has no C
  dependency and a small set of crates, and the `tar` crate needs `std::fs` paths that do
  not fit the `chroot` child's fd-relative calls.

`Sandbox::unpack` on the control side is `write_file`'s loop on the unpack calls
(`ExecFileCalls` gets the four calls).

### 6.7 The user of a command

`Command.user: Option<String>` and `ExecRequest.user`. The slave resolves it before
`fork()`: `"uid"`, `"uid:gid"`, `"name"`, `"name:group"`; names are looked up in
`<exec_root>/etc/passwd` and `/etc/group` (a small parser; no library call, because the
slave's own name service is not the image's). An unknown name gives `BadRequest`. With a
UID and no GID, the GID is the user's primary group from `passwd`, or the UID's number
when the file has no entry. The child calls `setgroups([])`, `setgid`, `setuid` after
`chroot` and before `execve`. When `HOME` is not in the environment, the slave sets it to
the user's home from `passwd`, or `/` when there is none.

### 6.8 Fault table: fill an image

| Step that fails | What happens | What the tool sees | What is left behind |
|---|---|---|---|
| `mkfs.ext4` missing or fails | No file. | `BinaryNotFound` or `Io` with the program's output tail | nothing |
| The sandbox does not start | The driver cleans up. | the start error | the blank work file; the tool removes it |
| An entry fails (no space, bad header) | The child stops; the slave ends the session. | `SlaveError { code: Io, errno, message: <entry path> }` | a half-filled work file; the tool removes it |
| The tool's stream ends early | `UnpackAbort`; the child is killed. | the stream error | the same |
| `Shutdown` cannot unmount | Remount read-only, then power off. | `Ok` | a clean file |
| The host dies during the fill | The VM dies with it. | (gone) | a work file in `images/work/`; `ImageStore::new` removes work files older than 24 hours at start (`ImageLimits.work_max_age`) |

## 7. Tests

| Law or scenario | Test | Where | Oracle |
|---|---|---|---|
| manifest | `manifest_roundtrip` | `tests/images_*.rs` | identity |
| I1, store | `commit_is_atomic`, `commit_same_key_keeps_first`, `tag_points_to_key`, `remove_removes_tag`, `work_files_expire` | the same (real files, temporary directory) | fault, expected outcome |
| `snapshot` | `snapshot_equal_bytes`, `snapshot_keeps_holes` (disk use of the copy is not more than the source plus 1 MiB) | the same | identity, bound |
| I9, the two manifest scenarios | `image_defaults_apply`, `spec_wins` | `tests/control_sim_*.rs` | expected outcome |
| I11 | `image_arch_mismatch` | the same | expected outcome |
| tar reader | `tar_reader_formats` (ustar, PAX long path, GNU long name, each entry type) against fixtures made with GNU tar and with Go's `archive/tar` (the fixture files and their commands are in `tests/fixtures/`); `tar_reader_property` (generated trees: write with the `tar` crate in the test, read with the slave's reader, compare) | `tests/slave_unpack_*.rs` | differential |
| I3, "A tar cannot write outside its target" | `unpack_escape` (needs root for `chroot`: runs in the microVM tests) and `unpack_path_rules` (the path function, no root) | `e2e_images.rs`, `tests/slave_unpack_*.rs` | expected outcome |
| I4 | `unpack_limits` | `e2e_images.rs` | bound |
| "Whiteouts remove earlier entries" | `unpack_whiteouts` | `e2e_images.rs` | expected outcome |
| I2, "The guest fills an image with the right owners" | `e2e_fill_owners` | `e2e_images.rs` (Firecracker; and libkrun when `blk` is true) | expected outcome |
| I6 | `e2e_writable_clean_unmount` (`dumpe2fs -h` on the file after the stop shows "clean"; returns with a message when `dumpe2fs` is not found) | the same | expected outcome |
| I7, "Two writers ..." | `writable_lock` (fake `HostFs`), `e2e_writable_lock` | sim and e2e | expected outcome |
| I8, "The same image runs on both ..." | `e2e_image_acceptance_firecracker`, `e2e_image_acceptance_libkrun` | `e2e_images.rs` | the shared scenarios |
| I1, "A committed image does not change" | `e2e_image_not_changed` | the same | conservation |
| I10, "A command runs as the image user" | `user_parse` (each form, unknown name), `e2e_exec_user` | sim and e2e | expected outcome |
| fault table | `fill_fault_matrix` (simulation: each row) | `tests/control_sim_*.rs` | fault |
| libkrun | `krun_config_disks`, `probe_reports_blk` | `tests/libkrun_sim_*.rs`, `bin/sandbox_krun/tests/` | example |

## 8. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox -p sandbox_krun --features "libkrun slave testing" --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --features "libkrun slave testing" --profile uat images_ slave_unpack control_sim libkrun_sim
cargo test -p foundation_platform_sandbox --features "libkrun slave testing" --profile uat e2e_images -- --nocapture
cargo check -p agent_slave --target x86_64-unknown-linux-musl --profile uat
```

## 9. Done when

- Each scenario of section 5 and each row of section 7 passes on Firecracker; the libkrun
  rows pass on a host whose libkrun has block support, and `progress.md` records the host.
- An image filled by the guest has the owners and modes of its tar.
- `agent-sandbox doctor` shows `mkfs.ext4` and, for libkrun, `blk`.
- The slave is still a static musl program.
