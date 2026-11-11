---
feature: "Guest artifacts"
status: "pending"
priority: "high"
model: "sonnet"
depends_on: ["00-protocol-and-api-types"]
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
| Alpine ext4 image | Firecracker | Built from the Alpine root directory. |
| Kernel | Firecracker | The Firecracker CI kernel. |
| Agent initramfs | Firecracker | Built in memory from the agent program. |

Only the Alpine root directory is on the path to M1. An agent can deliver part A first as
its own PR, then part B.

## 2. Module layout

`backends/foundation_platform_sandbox/src/shared/artifacts/`:

| File | Content |
|---|---|
| `mod.rs` | `ArtifactStore`, `ArtifactSpec`, `ArtifactError` mapping into `SandboxError`. |
| `manifest.rs` | The pinned list of artifacts: name, URL, SHA-256, size. |
| `fetch.rs` | Streamed download to a temporary file, with a hash check. |
| `tarball.rs` | Streamed unpack of a `.tar.gz` into a directory. |
| `cpio.rs` | A `newc` archive writer. |
| `ext4.rs` | Directory to ext4 image through `mkfs.ext4 -d`. |

## 3. Part A: store, fetch, Alpine root directory

### 3.1 `ArtifactStore`

```rust
pub struct ArtifactStore { /* ctx, host, root: PathBuf */ }

impl ArtifactStore {
    /// `root` is `HostConfig.cache_dir`.
    pub fn new(ctx: Ctx, host: SandboxHost) -> Self;

    /// Returns the path of a ready artifact. Downloads or builds it when the cache
    /// does not have it.
    pub async fn ensure(&self, spec: &ArtifactSpec) -> SandboxResult<PathBuf>;

    /// The same, with progress.
    pub fn ensure_with_progress(&self, spec: ArtifactSpec) -> ArtifactStream; // Stream<PathBuf, ArtifactProgress>

    pub async fn alpine_rootfs_dir(&self, arch: GuestArch) -> SandboxResult<PathBuf>;
    pub async fn alpine_rootfs_ext4(&self, arch: GuestArch, size_mib: u32) -> SandboxResult<PathBuf>;
    pub async fn firecracker_kernel(&self, arch: GuestArch) -> SandboxResult<PathBuf>;
    pub async fn agent_initramfs(&self, agent_path: &Path) -> SandboxResult<PathBuf>;
}

pub enum ArtifactProgress { Downloading { done: u64, total: Option<u64> }, Verifying, Unpacking, Building }
pub enum GuestArch { X86_64, Aarch64 }
```

Cache layout: `<cache_dir>/<kind>/<sha256-prefix-16>/<file-or-dir>`. A ready entry has a
marker file `.ready` beside it. `ensure` builds into `<entry>.tmp-<random>` (the suffix from
`ctx.rng()`) and renames it. The rename is the commit; two processes that build the same
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

## 4. Part B: initramfs, ext4 image, kernel

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

`agent_initramfs(agent_path)` writes these entries: directories `dev`, `proc`, `sys`,
`run`; the file `init` with the agent bytes. The modification time field is 0, so the same
agent gives the same archive. The cache key is the SHA-256 of the agent file.

The agent file is about 1 MiB. Reading it whole is allowed: it is small and its content is
the input of the next step (root rule 4).

### 4.2 `ext4`

```rust
pub async fn build_ext4(
    ctx: &Ctx, host: &SandboxHost, source_dir: &Path, size_mib: u32, out: &Path,
) -> SandboxResult<()>;
```

- Runs `mkfs.ext4 -q -F -d <source_dir> -E root_owner=0:0 <out> <size_mib>M` through
  `host.processes`. `-d` fills the image from a directory with no mount and no root
  rights (e2fsprogs 1.43 or newer).
- The program must exist. If `mkfs.ext4` is not found, the error is
  `BinaryNotFound { name: "mkfs.ext4" }`, and feature 07 reports it in the host check.
- This is Linux only in 0.1. Firecracker is Linux only, so macOS does not need it.
- On a failed exit, the error has the last `max_error_message` bytes of its output.
- `alpine_rootfs_ext4(arch, size_mib)` calls `alpine_rootfs_dir` and then `build_ext4`.
  The cache key is the Alpine hash plus `size_mib`.

### 4.3 Kernel

`firecracker_kernel(arch)` is `ensure` on the manifest entry. The file is used as it is.

## 5. Tests

`backends/foundation_platform_sandbox/tests/artifacts_*.rs`:

| Test | Needs | Proves |
|---|---|---|
| `cpio_matches_reference` | nothing | `write_newc` output for a fixed input is equal, byte for byte, to a fixture made with GNU `cpio -o -H newc` (the fixture is in `tests/fixtures/`, with the command in a comment). |
| `cpio_is_deterministic` | nothing | Two calls give the same bytes. |
| `cpio_kernel_accepts_it` | KVM | Covered by the microVM test of feature 06. Named here so the link is clear. |
| `fetch_checks_hash` | nothing | A local HTTP server (the test server of `foundation_netio` or `foundation_testing`) gives a body; a wrong expected hash fails and leaves no file. |
| `fetch_stops_at_limit` | nothing | A body larger than `max_download_bytes` fails before the end. |
| `fetch_reads_bounded_error` | nothing | A 500 answer with a 10 MiB body gives an error message of at most `max_error_message` bytes. |
| `store_is_atomic` | nothing | A build that fails leaves no entry and no `.ready` marker. A second `ensure` builds again. |
| `store_reuses_entry` | nothing | The second `ensure` does not fetch (the server counts requests). |
| `tar_rejects_escape` | nothing | An archive with `../x` fails with `ArtifactUnsafePath`. |
| `tar_stops_at_limits` | nothing | An archive above `max_unpacked_bytes` fails. |
| `alpine_dir_real` | network | The real Alpine archive unpacks, and `bin/busybox` exists. The test returns with a message when the network is not available. |
| `ext4_builds` | Linux, `mkfs.ext4` | A small directory becomes an image; `dumpe2fs -h` (or the magic bytes `0x53 0xEF` at offset 1080) confirms ext4. |

## 6. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --profile uat artifacts
```

## 7. Done when

- Part A: `alpine_rootfs_dir` returns a directory with a working Alpine tree on Linux and
  on macOS.
- Part B: `agent_initramfs`, `alpine_rootfs_ext4` and `firecracker_kernel` return ready
  files on Linux.
- Each download is streamed, hashed and bounded.
- `learnings.md` has the pinned versions and how each hash was made.
