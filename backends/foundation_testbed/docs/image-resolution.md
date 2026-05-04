# VM Image Resolution Pipeline

## Problem Context

The original `ensure_image()` implementation had a two-step resolution: check the local cache, then fall back to a Vagrant Cloud download. This meant **exported bootstrapped VMs** -- such as `windows-11-bootstrapped.qcow2` sitting in `/home/darkvoid/EweStore/Testbed/` -- were **never used by tests**. The framework always downloaded a fresh Vagrant box that had:

- No tools installed (no Rust, Node, VS Build Tools, etc.)
- No bootstrap marker file (`.testbed-bootstrapped`)
- No virtio drivers, WebView2, mise, OpenSSH, or other pre-configured software

Every test run triggered a full bootstrap cycle: 10+ minutes of OpenSSH setup, mise installation, VS Build Tools, virtio drivers, WebView2, and more.

The resolution pipeline was redesigned to support a **4-step chain** that lets pre-bootstrapped images be used directly, eliminating redundant bootstrap cycles.

---

## Resolution Chain

`ensure_image()` (in `src/import/mod.rs`, line 31) now resolves images in this priority order:

```
TESTBED_IMAGE_{PROFILE} env var
        |
        v
  Local cache (~/.cache/foundation_testbed/images/)
        |
        v
  Image stores ([[image_stores]] in testbed.toml)
        |
        v
  Vagrant Cloud / prebaked_url fallback
```

### Step 1: Environment Variable Override

**Env var:** `TESTBED_IMAGE_{PROFILE_NAME}` (profile name uppercased, hyphens converted to underscores)

Examples:
- `TESTBED_IMAGE_WINDOWS_BUILD=/home/darkvoid/EweStore/Testbed/windows-11-bootstrapped.qcow2`
- `TESTBED_IMAGE_UBUNTU_DEV=/path/to/custom-ubuntu.qcow2`

**Behavior:**
- The file is used **as-is** -- no copy, no download, no extraction.
- If the file is a compressed archive (`.qcow2.gz` or `.qcow2.xz`), it is auto-decompressed to the cache directory on first use (`src/export::decompress()` is called).
- Validates that the file exists and is larger than 100 MB (`MIN_IMAGE_SIZE` constant, line 16).
- Returns an error if the file is missing or too small (likely corrupted).

**Safety guarantees:**
- `evict()` (line 225) is a **no-op** when an env var override is active -- it will never delete a user-managed file.
- `is_cached()` (line 203) returns `true` if the env var path exists.
- `cached_size()` (line 267) returns the actual file size from the env var path.

**Code path:** Lines 39-65 in `src/import/mod.rs`.

### Step 2: Local Cache

**Path:** `~/.cache/foundation_testbed/images/{image_name}`

The cache path is computed by `VmProfile::image_cache_path()` which joins `image_cache_dir()` with `profile.image_name`.

**Behavior:**
- Checks for the uncompressed file first (e.g., `windows-build.qcow2`).
- Then checks compressed variants: `.qcow2.gz` and `.qcow2.xz` (the `COMPRESSED_EXTENSIONS` constant, line 19).
- If a compressed variant is found and is > 100 MB, it is returned. The caller (`ensure_image`, line 71) calls `ensure_decompressed()` which auto-decompresses in-place if needed.

**Code path:** Lines 67-72 (`find_cached_image()` at line 166).

### Step 3: Image Stores

**Source:** `[[image_stores]]` sections in `testbed.toml`, loaded by `load_image_stores()` in `src/config.rs`.

**Key change:** Previously, image stores were only consulted for macOS profiles. Now they are checked for **ALL profile types** (Windows, Linux, macOS).

**How it works:**

`try_download_from_store()` (line 108) iterates through each configured store looking for `{profile_name}.qcow2` (e.g., `windows-build.qcow2`).

For each store, `store_download_url_direct()` (line 148) builds a URL/path based on the store type:

| Store Type | Behavior |
|---|---|
| **Local** | Checks if `{destination}/{profile_name}.qcow2` exists on disk. If yes, copies it to the cache. |
| **Http** | Builds `https://{destination}/{profile_name}.qcow2` and attempts a curl download. Validates the result (> 100 MB). |
| **R2** / **S3** | **Not supported** for direct download. These require `rclone` or `aws` CLI to resolve presigned URLs. `store_download_url_direct()` returns `None`, so the store is skipped. |

**Fail-through:** If a store doesn't have the file or the download fails, the pipeline silently continues to the next store, eventually falling through to Vagrant Cloud.

**Code path:** Lines 74-77 (entry), lines 108-144 (`try_download_from_store()`), lines 148-161 (`store_download_url_direct()`).

### Step 4: Vagrant Cloud Fallback

This is the original behavior, unchanged.

**Flow:**
1. `resolve_image_url()` (line 367) checks if the profile has a hardcoded `prebaked_url`.
2. If not, queries the Vagrant Cloud API (`https://app.vagrantup.com/api/v2/box/{username}/{box_name}`) via `query_vagrant_cloud()` (line 502).
3. The Vagrant Cloud lookup maps known image names to vagrant boxes:
   - `windows-11-x86_64.qcow2` -> `gusztavvargadr/windows-11`
   - `ubuntu-24.04-x86_64.qcow2` -> `alvistack/ubuntu-24.04`
4. Downloads the file to a `.downloading` temp file.
5. Post-download processing:
   - If the file is a compressed qcow2 (`.gz`/`.xz`), decompresses it.
   - If the file is a Vagrant box (tar or gzip tar, detected by magic bytes `0x1f 0x8b` or `tar -tf`), extracts the qcow2 disk image from the archive using `extract_qcow2_from_box()` (line 308).
   - Otherwise, renames the temp file to the final cache path.
6. Validates the final image is > 100 MB.

**Code path:** Lines 79-100, URL resolution at lines 367-381, Vagrant Cloud at lines 502-539, Vagrant box extraction at lines 308-360.

---

## macOS Special Case

macOS profiles (`profile.name` starts with `"macos"`) use a different layout: a directory containing `BaseSystem.qcow2`, `opencore.qcow2`, and `macOS-data.qcow2`. These are handled by `ensure_macos_from_store_or_fallback()` (line 388), which:

1. Checks if the macOS image directory is already cached.
2. Tries to download a pre-baked tarball (`.tar.gz`) from image stores or `prebaked_url` via `resolve_macos_tarball_url()` (line 432).
3. Falls back to native IPSW -> BaseSystem -> qcow2 creation via `macos::ensure_macos_image()`.

---

## Code Changes Summary

### `src/import/mod.rs`

| Change | Details |
|---|---|
| `ensure_image()` rewritten | 4-step resolution chain replacing the old 2-step (cache -> download) |
| `try_download_from_store()` added | Iterates `[[image_stores]]` looking for `{profile_name}.qcow2` |
| `store_download_url_direct()` added | Builds URL/path for Http and Local stores; returns None for R2/S3 |
| `find_cached_image()` added | Checks uncompressed, then `.gz` and `.xz` variants |
| `is_compressed_qcow2()` added | Checks `.gz`/`.xz` extensions for auto-decompress decision |
| `is_cached()` updated | Checks env var override before cache |
| `evict()` updated | No-op when env var override is active (won't delete user files) |
| `cached_size()` updated | Returns size of env var override file when set |

### `src/export/mod.rs`

| Change | Details |
|---|---|
| Export output updated | After export, prints both the `TESTBED_IMAGE_{PROFILE}=...` env var and `ewe_platform testbed adopt {name} --disk ...` CLI instructions (lines 174-183) |

---

## Usage Examples

### Environment Variable Override (immediate, no copy)

```bash
TESTBED_IMAGE_WINDOWS_BUILD=/home/darkvoid/EweStore/Testbed/windows-11-bootstrapped.qcow2 \
  cargo test -p foundation_testbed --test e2e_tauri test_tauri_build_windows -- --ignored
```

The file is used directly. No copy is made. The bootstrap marker inside the VM disk is preserved, so the bootstrap step is skipped entirely.

### Compressed Image Override (auto-decompressed on first use)

```bash
TESTBED_IMAGE_LINUX_BUILD=/path/to/linux-build.qcow2.gz \
  cargo test -p foundation_testbed --test e2e_tauri test_tauri_build_linux -- --ignored
```

On first use, the `.gz` file is decompressed to `~/.cache/foundation_testbed/images/linux-build.qcow2`. Subsequent runs use the decompressed cache directly.

### Local Image Store (testbed.toml)

Add to `testbed.toml`:

```toml
[[image_stores]]
name = "ewestore"
store_type = "Local"
destination = "/home/darkvoid/EweStore/Testbed"
```

Place your exported images in that directory with the naming convention `{profile_name}.qcow2`:

```
/home/darkvoid/EweStore/Testbed/
  windows-build.qcow2
  ubuntu-dev.qcow2
  macos-15.tar.gz        # macOS uses tarball format
```

Then tests automatically find and copy the image from that directory to the cache.

### HTTP Image Store

```toml
[[image_stores]]
name = "ci-store"
store_type = "Http"
destination = "https://ci.example.com/images"
```

The pipeline will attempt to download `https://ci.example.com/images/windows-build.qcow2`.

### Export + Import Workflow

```bash
# 1. Bootstrap a VM (one-time, takes ~10 min)
ewe_platform testbed bootstrap windows-build

# 2. Export it as a pre-bootstrapped image
ewe_platform testbed export windows-build --version bootstrapped

# Output:
#   Export complete: .build/export/windows-build-bootstrapped.qcow2
#   Import (env override): export TESTBED_IMAGE_WINDOWS_BUILD=.build/export/windows-build-bootstrapped.qcow2
#   Import (adopt): ewe_platform testbed adopt windows-build --disk .build/export/windows-build-bootstrapped.qcow2

# 3. Use the exported image in tests (bootstrap skipped)
TESTBED_IMAGE_WINDOWS_BUILD=.build/export/windows-build-bootstrapped.qcow2 \
  cargo test -p foundation_testbed --test e2e_tauri -- --ignored
```

---

## Why This Matters

The bootstrap marker (`.testbed-bootstrapped`) lives **inside the VM disk image**. Without the ability to point testbed at a pre-bootstrapped image, every test run:

1. Downloads a pristine Vagrant box (no tools, no bootstrap marker)
2. Runs the full bootstrap sequence:
   - OpenSSH installation and configuration
   - mise (version manager) setup
   - VS Build Tools (Windows, ~4 GB download)
   - virtio drivers installation
   - WebView2 runtime (Windows)
   - Rust toolchain, Node.js, and other development tools
   - Bootstrap marker creation

This takes **10+ minutes** per test run. With the environment variable override or image stores, the bootstrapped image is used directly and **the entire bootstrap step is skipped**, reducing test startup from 10+ minutes to seconds.

---

## File Reference

| File | Role |
|---|---|
| `backends/foundation_testbed/src/import/mod.rs` | Image import orchestrator -- `ensure_image()` and 4-step chain |
| `backends/foundation_testbed/src/export/mod.rs` | VM export -- produces qcow2 + manifest, prints import instructions |
| `backends/foundation_testbed/src/export/compress.rs` | Compression utilities (gzip/xz decompress) |
| `backends/foundation_testbed/src/config.rs` | `ImageStore`, `StoreType`, `VmProfile::image_cache_path()`, `load_image_stores()` |
| `backends/foundation_testbed/src/qemu/download.rs` | Low-level curl-based download (`download::download()`, `download::is_cached()`) |
| `~/.cache/foundation_testbed/images/` | Default image cache directory (`image_cache_dir()`) |
