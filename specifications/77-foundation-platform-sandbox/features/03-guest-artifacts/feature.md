---
feature: "Guest artifacts"
status: "pending"
priority: "high"
model: "sonnet"
phase: "MVP"
depends_on: ["00-contract"]
crates: ["foundation_platform_sandbox"]
---

# Feature 03: Guest artifacts (kernel, root file systems, initramfs)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Secure Hash Algorithm 256 (SHA-256); Hypertext Transfer
Protocol (HTTP); Open Container Initiative (OCI); virtual machine (VM); continuous
integration (CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100.

## 1. Goal

A sandbox needs files for the guest. This feature makes them available on the host, in a
cache, with a checked hash:

| Artifact | For | Source |
|---|---|---|
| Alpine root directory | libkrun | The Alpine "minirootfs" release archive, unpacked. |
| Ubuntu squashfs image | Firecracker | The Firecracker CI root file system. |
| Kernel | Firecracker | The Firecracker CI kernel. |
| Slave initramfs | Firecracker | Built in memory from the `agent-slave` program. |

Part B (kernel, Firecracker root image, initramfs) is on the path to MVP-A. Part A also
gives the Alpine root directory, which the libkrun driver (feature 09) and the ext4 test of
feature 05 use. One PR can deliver both parts.

## A. Story

- **S1**: "A caller asks for a guest file (a kernel, a root file system, an initramfs) and
  gets a path to a verified copy."
- **S2**: "A second caller asks for the same file and no download happens."
- **S3**: "The download is cut, is too large, or has the wrong hash."
- **S4**: "An archive tries to write outside its target directory, or expands without end."
- **S5**: "Two processes ask for the same file at the same time."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Get a pinned artifact | driver, test, program | `ensure(spec)` | The path of a ready entry. |
| U2 | Get the Alpine directory | libkrun user | `alpine_rootfs_dir` | An unpacked tree. |
| U3 | Get the Firecracker kernel and root | Firecracker user | `firecracker_kernel`, `firecracker_ci_rootfs` | Two verified files. |
| U4 | Build the slave initramfs | Firecracker driver | `slave_initramfs(slave)` | A `newc` archive with the slave as `/init`. |

## B. Bounded context

**Inside this feature:** the cache of guest files on the host, and the code that fills it.

**Outside:** who uses the files (features 05, 06, 09, 12 to 15); the HTTP client
(`foundation_netio`); the slave program (features 01 and 06, an input here).

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Artifact spec | Name, URL, SHA-256, kind. Data. | `artifacts::manifest` (pinned ones); the caller (its own) |
| Cache entry | A directory `<kind>/<hash prefix>/` with the file or tree and a `.ready` marker. | `ArtifactStore` only |
| Temporary entry | `<entry>.tmp-<random>`, visible to one build. | the `ensure` call that made it |
| `newc` archive | The initramfs bytes. | `artifacts::cpio` |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| ensure | story | a spec | Returns a ready entry; fetches and builds it when it is missing. |
| fetch | entity | a URL | Streams a body to a temporary file while it hashes. |
| verify | entity | a file and a hash | Accepts or removes the file. |
| unpack | entity | a `.tar.gz` | Writes a tree, inside limits. |
| commit | entity | a temporary entry | Renames it to the entry and writes `.ready`. |
| write_newc | entity | entries | Writes archive bytes. |

## C. Rules

### Law worksheet

```
Entity: Cache entry
Conserved value:         the content of a ready entry never changes
Bound:                   download <= max_download_bytes; unpacked bytes <= max_unpacked_bytes; entries <= max_entries
Inverse pairs:           none (the store has no delete in this feature)
Idempotent verbs:        ensure (a second call returns the same path and does no work)
Ordering pairs:          fetch -> verify -> commit (each enables the next); two ensure calls for one spec compete, and both return a correct entry
Monotone values:         an entry moves missing -> ready, never back
Freeze points:           content at commit
Links to other entities: the entry path contains the first 16 hex digits of the spec hash

Entity: newc archive
Conserved value:         the archive holds exactly the given entries plus the trailer
Bound:                   total length is a multiple of 512
Inverse pairs:           write_newc / GNU cpio -i (the reference reader gives the entries back)
Idempotent verbs:        write_newc (same input, same bytes)
Ordering pairs:          entries appear in the given order; the trailer is last
Monotone values:         none
Freeze points:           modification time field is 0
Links to other entities: cache key = SHA-256 of the slave file
```

### Invariants

| # | Invariant |
|---|---|
| I1 | A path that `ensure` returns holds bytes whose SHA-256 equals the spec hash (for a tree: the hash of its source archive). |
| I2 | A failed `ensure` leaves no entry, no `.ready` marker and no temporary entry. |
| I3 | A body is streamed. Memory use does not grow with the body size. |
| I4 | A download stops at `max_download_bytes`; an unpack stops at `max_unpacked_bytes` and `max_entries`. |
| I5 | No unpacked path is outside the target directory. |
| I6 | `write_newc` gives the same bytes for the same input. |
| I7 | Two `ensure` calls for one spec, at the same time, both return a complete entry. |
| I8 | A non-200 answer reads at most `max_error_message` bytes of its body. |

### Must have

- `ArtifactStore`, the manifest, `fetch`, `tarball`, `cpio`.
- `ArtifactLimits` with `max_download_bytes`, `max_unpacked_bytes`, `max_entries`.

### Must do

- Check the status before the body.
- Remove a temporary entry on every failure path.
- Record in `learnings.md` how each pinned hash was made.

### Must not

- Must not list a remote bucket at run time. Versions are pinned.
- Must not unpack a device node, or follow an archive path with `..`.
- Must not read a whole download into memory.

## D. Acceptance scenarios

```gherkin
Feature: Guest artifacts

  Background:
    Given an empty cache directory and a local HTTP server

  Scenario: A verified download becomes a ready entry
    Given the server gives 1 MiB of bytes with a known hash
    When ensure is called with that hash
    Then the returned path holds those bytes
    And a .ready marker exists

  Scenario: The second request does no work
    Given a ready entry
    When ensure is called again
    Then the server got no new request

  Scenario: A wrong hash leaves nothing
    When ensure is called with a hash that does not match
    Then the result is ArtifactHashMismatch
    And the cache directory has no entry and no temporary entry

  Scenario: A download that is too large is cut
    Given max_download_bytes is 1024 and a body of 1 MiB
    When ensure is called
    Then the result is ArtifactTooLarge
    And the client read less than 64 KiB of the body

  Scenario: An error answer is read with a bound
    Given the server answers 500 with a 10 MiB body
    When ensure is called
    Then the error message is at most max_error_message bytes

  Scenario: An archive cannot write outside its target
    Given an archive with the entry "../x"
    When it is unpacked
    Then the result is ArtifactUnsafePath
    And no file "x" exists beside the target

  Scenario: An archive that expands too far is cut
    Given max_unpacked_bytes is 1 MiB and an archive that expands to 100 MiB
    When it is unpacked
    Then the result is ArtifactTooLarge

  Scenario: Two builders do not corrupt an entry
    When two ensure calls for one spec run at the same time
    Then both return the same path with the correct bytes

  Scenario: The initramfs is the same each time
    When slave_initramfs is called two times for one slave file
    Then the two archives are byte-equal
    And GNU cpio unpacks it into the same entries
```

| Law | Test | Oracle |
|---|---|---|
| I1 | `fetch_checks_hash`, `alpine_dir_real`, `ci_rootfs_real` | expected outcome |
| I2 | `fetch_streams` | nothing | A 64 MiB body is fetched while a counting writer shows that no buffer larger than 1 MiB is held (invariant I3). |
| `store_concurrent_ensure` | nothing | Two `ensure` calls for one spec in two tasks both return the same complete entry (invariant I7). |
| `store_is_atomic` | fault (the server cuts the body; a bad hash) |
| I3 | `fetch_streams` (a 64 MiB body with a 1 MiB memory budget on the test allocator counter) | bound |
| I4 | `fetch_stops_at_limit`, `tar_stops_at_limits` | bound |
| I5 | `tar_rejects_escape` (examples: `../x`, `/abs`, `a/../../x`) | expected outcome |
| I6 | `cpio_is_deterministic`, `cpio_golden`, `cpio_read_by_gnu_cpio` | identity, differential against GNU `cpio` |
| I7 | `store_concurrent_ensure` | property over interleavings (two valtron tasks) |
| I8 | `fetch_reads_bounded_error` | bound |

## 2. Module layout

`backends/foundation_platform_sandbox/src/native/artifacts/`:

| File | Content |
|---|---|
| `mod.rs` | `ArtifactStore`, `ArtifactSpec`, `ArtifactError` mapping into `SandboxError`. |
| `manifest.rs` | The pinned list of artifacts: name, URL, SHA-256, size. |
| `fetch.rs` | Streamed download to a temporary file, with a hash check. |
| `tarball.rs` | Streamed unpack of a `.tar.gz` into a directory. |
| `cpio.rs` | A `newc` archive writer. |

## 3. Part A: store, fetch, Alpine root directory

### 3.1 `ArtifactStore`

```rust
pub struct ArtifactStore { /* ctx, fs, root: PathBuf */ }

impl ArtifactStore {
    /// `fs` is the host file system; `cache_dir` is `HostConfig.cache_dir`.
    pub fn new(ctx: Ctx, fs: Arc<dyn HostFs>, cache_dir: PathBuf) -> Self;

    /// Returns the path of a ready artifact. Downloads or builds it when the cache
    /// does not have it.
    pub async fn ensure(&self, spec: &ArtifactSpec) -> SandboxResult<PathBuf>;

    /// The same, with progress.
    pub fn ensure_with_progress(&self, spec: ArtifactSpec) -> ArtifactStream; // Stream<PathBuf, ArtifactProgress>

    pub async fn alpine_rootfs_dir(&self, arch: GuestArch) -> SandboxResult<PathBuf>;
    /// The Alpine archive file itself (`.tar.gz`), for the image importer of feature 13.
    pub async fn alpine_rootfs_archive(&self, arch: GuestArch) -> SandboxResult<PathBuf>;
    pub async fn firecracker_ci_rootfs(&self, arch: GuestArch) -> SandboxResult<PathBuf>;   // squashfs
    pub async fn firecracker_kernel(&self, arch: GuestArch) -> SandboxResult<PathBuf>;
    pub async fn slave_initramfs(&self, slave_path: &Path) -> SandboxResult<PathBuf>;
}

pub enum ArtifactProgress { Downloading { done: u64, total: Option<u64> }, Verifying, Unpacking, Building }
pub enum GuestArch { X86_64, Aarch64 }
```

The store needs only files and HTTP, not processes. This feature writes `NativeFs`, the
real `HostFs`, as the first content of `src/native/host.rs`; feature 05 adds the process
spawner and the connector to that file. `HostFs` gets two more methods for this feature:
`rename(from, to)` and `create_file(path) -> SandboxResult<Box<dyn std::io::Write + Send>>`
(a file for streamed writes). The HTTP client is `foundation_netio` and is not behind a
trait of this crate: tests use a local HTTP server.

`ArtifactSpec { name: String, url: String, sha256: String, kind: ArtifactKind }`, with
`ArtifactKind::File` (the entry is the downloaded file) and `ArtifactKind::TarGzDir` (the
entry is the unpacked tree, and the hash is of the archive). `ArtifactLimits` is a field of
`HostConfig` (`artifact_limits`), with the defaults that this feature names.

The store is native code (`src/native/artifacts/`): it uses the HTTP client, `flate2` and
`tar`, and nothing on wasm needs it.

Cache layout: `<cache_dir>/<kind>/<sha256-prefix-16>/<file-or-dir>`. A ready entry has a
marker file `.ready` beside it. `ensure` builds into `<entry>.tmp-<random>` (the suffix from
`ctx.rng()`) and renames it. The rename is the commit (the `.ready` marker is written inside the temporary entry before
the rename, so an entry that exists is complete; if the rename fails because the entry
exists, another builder won, and this one removes its temporary copy); two processes that build the same
entry do not corrupt it, and the loser removes its temporary copy.

### 3.2 `fetch`

- Uses `foundation_netio` (`HttpClientBuilder`, `DynNetClient`). Read how
  `backends/foundation_deployment_platform/src/qemu/download.rs` downloads images, and use
  the same client calls.
- Checks the status first. A status that is not 200 reads at most `max_error_message`
  bytes for the message (root rule 4).
- Streams the body to the temporary file in chunks. It feeds each chunk to a SHA-256
  hasher (`sha2`, already in the workspace). It never holds the body in memory.
- Fails with `ArtifactHashMismatch { expected, actual }` and removes the file when the
  hash differs.
- The download size has a limit, `ArtifactLimits.max_download_bytes` (default 1 GiB; the
  largest pinned artifact is the kernel at about 40 MiB, and a user image can be larger).
  The stream stops when it passes the limit.

### 3.3 Manifest

`manifest.rs` has one table for each architecture. The implementer fills the values and
records in `learnings.md` how each hash was made.

| Name | URL pattern | Notes |
|---|---|---|
| `alpine-minirootfs` | `https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/<arch>/alpine-minirootfs-3.22.<patch>-<arch>.tar.gz` | Use the newest 3.22 patch release on the day of the work. Alpine publishes a `.sha256` file beside the archive; compare with it. |
| `firecracker-kernel` | `https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/<prefix>/<arch>/vmlinux-6.1.<patch>` | Find `<prefix>` with the list call in `learnings.md` section 2.6, one time, by hand. Pin it. Compute the SHA-256 of the downloaded file. |
| `firecracker-ci-rootfs` | `https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/<prefix>/<arch>/ubuntu-<version>.squashfs` | The same `<prefix>`. A squashfs image with root-owned files. |

The manifest is data, not code paths: a caller can pass its own `ArtifactSpec { name, url,
sha256, kind }` to `ensure`.

### 3.4 `tarball`

- Streams the file through `flate2` and `tar` (both in the workspace).
- Unpacks into the temporary directory. For each entry it checks that the path has no
  `..` part and is not absolute; a bad entry fails the unpack (`ArtifactUnsafePath`).
- It keeps file modes. It does not need root: device nodes in the archive are skipped
  with a `tracing::debug!` line (the Alpine minirootfs has none), and the owner is the
  current user. The guest sees files as root because virtio-fs and `mkfs.ext4 -d`
  (section 4.2) set the owner.
- Limits: `ArtifactLimits.max_unpacked_bytes` (default 4 GiB) and `max_entries` (default
  1 000 000). Passing one fails the unpack. These stop an archive that expands without
  end.

## 4. Part B: initramfs, kernel, Firecracker root file system

### 4.1 `cpio`

The initramfs has one program. The kernel accepts an uncompressed `newc` archive.

```rust
pub struct CpioEntry<'a> { pub path: &'a str, pub mode: u32, pub data: &'a [u8] }

/// Writes a `newc` archive: the entries, then the `TRAILER!!!` entry, then zero
/// bytes up to a multiple of 512.
pub fn write_newc(entries: &[CpioEntry<'_>], out: &mut impl std::io::Write) -> std::io::Result<()>;
```

`newc` format, for each entry: the 6 bytes `070701`, then 13 fields of 8 upper-case hex
digits each (inode, mode, user ID, group ID, link count, modification time, file size,
device major, device minor, device major of a device node, device minor of a device node,
name size with the zero byte, check sum = 0), then the name with a zero byte, then zero
bytes up to a multiple of 4, then the data, then zero bytes up to a multiple of 4. A
directory has mode `040755` and no data. A file has mode `0100755`.

`slave_initramfs(slave_path)` writes these entries: directories `dev`, `proc`, `sys`,
`run`; the file `init` with the slave bytes. The modification time field is 0, so the same
slave gives the same archive. The cache key is the SHA-256 of the slave file.

The slave file is a few MiB. Reading it whole is allowed: it is small and its content is
the input of the next step (root rule 4).

### 4.2 Firecracker root file system

`firecracker_ci_rootfs(arch)` is `ensure` on the manifest entry `firecracker-ci-rootfs`. The
file is used as it is: Firecracker attaches it read-only, and the slave mounts it as the
lower layer.

Why not an ext4 image made from the Alpine directory: `mkfs.ext4 -d <directory>` copies the
host owner of each file into the image, so a normal user gets an image where no file is
owned by root. Feature 12 makes ext4 images with correct owners. Version 0.1 tests
Firecracker with the squashfs image, which already has correct owners.

### 4.3 Kernel

`firecracker_kernel(arch)` is `ensure` on the manifest entry. The file is used as it is.

## 5. Tests

`backends/foundation_platform_sandbox/tests/artifacts_*.rs`:

| Test | Needs | Proves |
|---|---|---|
| `cpio_golden` | nothing | `write_newc` output for a fixed input is equal, byte for byte, to a golden file in `tests/fixtures/` that this code made and a person checked one time with `cpio -t`. The header fields are fixed by rule: inode = the entry index from 1; link count 2 for a directory and 1 for a file; user and group 0; modification time 0; device numbers 0. |
| `cpio_read_by_gnu_cpio` | `cpio` program | GNU `cpio -i` unpacks the archive into the same entries (names, modes, content). Returns with a message when `cpio` is not found. |
| `cpio_is_deterministic` | nothing | Two calls give the same bytes. |
| `cpio_kernel_accepts_it` | KVM | Covered by the microVM tests of feature 05. Named here so the link is clear. |
| `fetch_checks_hash` | nothing | A local HTTP server (the test server of `foundation_netio` or `foundation_testing`) gives a body; a wrong expected hash fails and leaves no file. |
| `fetch_stops_at_limit` | nothing | A body larger than `max_download_bytes` fails before the end. |
| `fetch_reads_bounded_error` | nothing | A 500 answer with a 10 MiB body gives an error message of at most `max_error_message` bytes. |
| `store_is_atomic` | nothing | A build that fails leaves no entry and no `.ready` marker. A second `ensure` builds again. |
| `store_reuses_entry` | nothing | The second `ensure` does not fetch (the server counts requests). |
| `tar_rejects_escape` | nothing | An archive with `../x` fails with `ArtifactUnsafePath`. |
| `tar_stops_at_limits` | nothing | An archive above `max_unpacked_bytes` fails. |
| `alpine_dir_real` | network | The real Alpine archive unpacks, and `bin/busybox` exists. The test returns with a message when the network is not available. |
| `ci_rootfs_real` | network | The real squashfs file downloads, its hash matches, and its first 4 bytes are `hsqs`. Returns with a message when the network is not available. |

## 6. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --profile uat artifacts
```

## 7. Done when

- Part A: `alpine_rootfs_dir` returns a directory with a working Alpine tree on Linux and
  on macOS.
- Part B: `slave_initramfs`, `firecracker_ci_rootfs` and `firecracker_kernel` return ready
  files on Linux.
- Each download is streamed, hashed and bounded.
- `learnings.md` has the pinned versions and how each hash was made.
