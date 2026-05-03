---
feature: "VM Export & Distribution"
description: "Export running or stopped VMs as pre-built qcow2 images with metadata, upload to R2/S3/GitHub Releases, generate import manifests for distribution"
status: "pending"
priority: "high"
depends_on: ["05-cli-state-management", "08-provider-architecture"]
estimated_effort: "medium"
created: 2026-05-03
last_updated: 2026-05-03
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# VM Export & Distribution Feature

## Overview

After building and configuring a VM (installing tools, bootstrapping, customizing),
users can export it as a pre-built qcow2 image with metadata and upload it to
remote storage (R2, S3, GitHub Releases). This enables teams to share
pre-configured VMs — new team members get a fast boot (~2 min to SSH) instead
of a full bootstrap cycle (15-25 min).

The export command is the **inverse of import**: it packages what import
downloads.

## Why This Matters

| Use case | Why |
|----------|-----|
| Team onboarding | New dev runs `testbed start` with a pre-baked image — no bootstrap wait |
| CI reproducibility | CI pipeline pulls a known-good VM image, builds are deterministic |
| macOS image distribution | Create macOS image once (slow), export and share with team |
| Golden images | Export a "golden" VM with specific tool versions, distribute to all devs |
| Self-hosted image registry | Organizations host their own images on R2/S3 instead of Vagrant Cloud |

## Requirements

### 10.1 Export Command

```
ewe_platform testbed export <name> [options]
```

**Options:**
- `--out <path>` — local file path for exported image (default: `.build/export/`)
- `--upload <destination>` — remote destination: `s3://bucket/key`, `r2://bucket/key`, or `gh-release://owner/repo/tag`
- `--include-bootstrap` — include bootstrap marker (VM is pre-bootstrapped)
- `--clean` — clean up VM internal state (temp files, caches) before export
- `--shrink` — zero-fill free space and compress qcow2 before export (smaller file)

**Behavior:**
1. **Stop VM if running** (optional: `--running` exports live state)
2. **Prepare disk image:**
   - Copy current qcow2 disk to export location
   - If `--clean`: zero-fill free space (`fstrim` inside VM before stop)
   - If `--shrink`: `qemu-img convert -O qcow2 -c` (compressed)
3. **Generate manifest** (`manifest.json`) alongside the qcow2
4. **If `--upload`**: upload qcow2 + manifest to remote storage
5. **Print import instructions** — the exact `testbed import` command to use the exported image

### 10.2 Export Manifest

Each exported image includes a `manifest.json` with metadata:

```json
{
  "name": "linux-build",
  "version": "1.0.0",
  "os": "linux",
  "arch": "x86_64",
  "image_file": "linux-build-x86_64-1.0.0.qcow2",
  "image_size_bytes": 4294967296,
  "image_sha256": "abc123...",
  "bootstrap_version": 3,
  "installed_tools": {
    "rust": "1.78.0",
    "node": "22.1.0",
    "tauri-cli": "2.0.0",
    "mise": "2024.5.1",
    "nu": "0.93.0"
  },
  "created_at": "2026-05-03T14:30:00Z",
  "created_by": "alex@laptop",
  "git_commit": "59ba299f",
  "notes": "Base image with rust, tauri-cli, node 22, and VS Code server"
}
```

This manifest is used by the import command to:
- Verify image integrity (SHA256 check)
- Show installed tools to the user before boot
- Determine if re-bootstrap is needed

### 10.3 Export to Image Store

Export uploads to a named store from `testbed.toml`'s `[[image_stores]]`:

```bash
testbed export linux-build --store alex_r2
testbed export linux-build --store team_s3
testbed export linux-build --store local_disk
```

The store must be type `r2`, `s3`, `local`, or `http` — `vagrant` stores are
read-only and can't be export targets.

**Upload methods by store type:**

| Type | Upload command |
|------|---------------|
| `r2` | `rclone copy` (assumes user has rclone configured) |
| `s3` | `aws s3 cp` (assumes user has aws CLI configured) |
| `local` | `std::fs::copy` |
| `http` | `simple_http` PUT with optional auth headers |

**Example:**
```bash
# Export to R2 store defined in testbed.toml
testbed export linux-build --store alex_r2 --shrink

# Export to local disk store
testbed export linux-build --store local_cache --version 1.0.0
```

### 10.4 Import from Exported Image

The existing `import` command resolves images via `[[image_stores]]` in
`testbed.toml`. When a store contains an exported image:

1. Checks `$HOME/.testbed/images/` (already cached) → skip download
2. Iterates `image_stores` in order — first store with the image wins
3. Downloads the qcow2 to `$HOME/.testbed/images/`
4. Looks for `manifest.json` in the same store location
5. If manifest found, displays installed tools and bootstrap version
6. Verifies SHA256 matches

### 10.5 Disk Shrink & Optimization

Before export, the VM disk can be optimized:

**Shrink flow (requires VM running):**
```
1. Inside VM: fstrim / (Linux) or Optimize-Volume -DriveLetter C (Windows)
2. Inside VM: dd if=/dev/zero of=/zero.fill bs=1M; rm /zero.fill (Linux)
   OR Windows: sdelete64 -z C:\ (Sysinternals, installed during bootstrap)
3. Stop VM
4. qemu-img convert -O qcow2 -c input.qcow2 output.qcow2
```

The `--shrink` flag performs all steps. Without it, the qcow2 is copied
as-is (faster but larger file).

### 10.6 CLI Integration

Add to the CLI command tree:

```
testbed export <name>
├── --store <name>            # Upload to named image_store from testbed.toml
├── --out <path>              # One-off local export path (bypasses stores)
├── --include-bootstrap       # Mark as pre-bootstrapped
├── --clean                   # Zero-fill free space before export
├── --shrink                  # Compress qcow2 (slower, smaller)
└── --version <semver>        # Version tag for the export
```

## Implementation Phases

### Phase 1: Local Export (Tasks 1-3)
1. Create `src/export/mod.rs` — orchestrator: stop VM, copy disk, generate manifest
2. Create `src/export/shrink.rs` — fstrim, zero-fill, qemu-img compress
3. Create `src/export/manifest.rs` — manifest generation with installed tools detection

### Phase 2: Upload (Tasks 4-5)
4. Create `src/export/store_upload.rs` — upload to image stores (rclone, aws CLI, simple_http PUT)
5. Create `src/export/destination.rs` — resolve store by name from config, validate store type

### Phase 3: Import Extension (Tasks 7-8)
7. Extend `import::download` to read and verify manifest.json alongside qcow2
8. Add manifest display on import: "This image has: rust 1.78, node 22, tauri 2.0"

## Success Criteria

- [ ] `ewe_platform testbed export linux-build --out ./export.qcow2` exports a valid qcow2 + manifest.json
- [ ] `ewe_platform testbed export linux-build --store alex_r2` uploads to R2 via rclone
- [ ] `ewe_platform testbed export linux-build --store team_s3` uploads to S3 via aws CLI
- [ ] `ewe_platform testbed export linux-build --shrink` produces a smaller compressed qcow2
- [ ] Exported images can be imported via `testbed.toml` `[[image_stores]]` entries
- [ ] Manifest shows installed tools and bootstrap version on import
- [ ] SHA256 verification catches corrupted downloads

## Verification Commands

```bash
# Export locally
ewe_platform testbed export linux-build --out /tmp/test.qcow2
ls -lh /tmp/test.qcow2 /tmp/manifest.json

# Export with shrink
ewe_platform testbed export linux-build --out /tmp/test-shrunk.qcow2 --shrink

# Export to R2 store (defined in testbed.toml)
ewe_platform testbed export linux-build --store alex_r2

# Export to S3 store
ewe_platform testbed export linux-build --store team_s3 --version 1.0.0
```

---

## Implementation Plan

### Export Flow

```
User: testbed export linux-build --store alex_r2

1. Load VM state → find profile, disk path, running status
2. If VM running: optionally fstrim + zero-fill (--shrink), then stop
3. Copy qcow2 to temp location
4. If --shrink: qemu-img convert -O qcow2 -c temp.qcow2 shrunk.qcow2
5. Generate manifest.json:
   - Read disk image size, compute SHA256
   - Query VM for installed tools (via SSH: rustc -V, node -V, etc.)
   - Include git commit, timestamp, creator
6. Upload to store "alex_r2":
   - type=r2 → rclone copy shrunk.qcow2 my-r2-remote:bucket/ + rclone copy manifest.json
7. Print: "Exported linux-build v1.0.0 → store alex_r2"
   Print: "Import: ensure [[image_stores]] includes alex_r2, then testbed start linux-build"
```

### Image Store Upload

Each store type delegates to its CLI tool:

```rust
fn upload_to_store(store: &ImageStore, local_path: &Path, manifest_path: &Path) -> Result<()> {
    match store.store_type {
        StoreType::R2 => {
            // rclone copy local_path r2-remote:bucket/
            Command::new("rclone").args([...]).status()?
        }
        StoreType::S3 => {
            // aws s3 cp local_path s3://bucket/key
            Command::new("aws").args([...]).status()?
        }
        StoreType::Local => {
            // std::fs::copy
            fs::copy(local_path, &store.directory)?
        }
        StoreType::Http => {
            // simple_http PUT with optional auth
            client.put(store.base_url, file_bytes)?
        }
    }
}
```

R2 and S3 stores assume the user has `rclone`/`aws` CLI configured with
credentials. We don't manage credentials — we just invoke the CLI with the
right bucket/path arguments from the store config.

### Installed Tools Detection

Query the VM via SSH (nushell) before stopping:

```nu
# Linux
rustc --version | awk '{print $2}'
node --version | sed 's/^v//'
cargo binstall --version
nu --version
mise --version

# Windows
(rustc --version).Split(' ')[1]
(node --version).TrimStart('v')
```

The export manifest captures these versions so importers know what's pre-installed.

### Disk Size Comparison

| VM type | Raw qcow2 | After fstrim+compress |
|---------|-----------|-----------------------|
| Linux (bootstrapped, empty) | 4 GB | 800 MB |
| Windows (bootstrapped, empty) | 30 GB | 4 GB |
| Linux (after cargo build) | 12 GB | 2 GB |

The `--shrink` flag is important for distribution — raw qcow2 files are
much larger than their actual data content due to QEMU's dynamic allocation
tracking.
