---
feature: "Bin Integration"
description: "Wire foundation_testbed into bin/platform testbed subcommands, add UI testing automation, write README documentation"
status: "pending"
priority: "high"
depends_on: ["cli-state-management"]
estimated_effort: "medium"
created: 2026-05-02
last_updated: 2026-05-02
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# Bin Integration Feature

## Overview

The final integration layer: wires `foundation_testbed` into the `bin/platform` CLI as a `testbed` subcommand group, adds UI testing automation capabilities (click, type, validate), and writes comprehensive README documentation.

## Dependencies

- Depends on **05-cli-state-management** (all CLI commands and logic are in place)
- Uses `foundation_core` and `foundation_errstacks`

## Requirements

### 6.1 bin/platform Testbed Subcommands

Add `bin/platform/src/testbed/mod.rs` — a thin CLI wrapper that:

1. Registers all `ewe_platform testbed <subcommand>` commands via `clap`
2. Parses CLI arguments
3. Calls into `foundation_testbed` public API functions
4. Prints results/errors to stdout/stderr

**Zero business logic** in the binary. All logic is in `foundation_testbed`.

The `bin/platform` binary uses `tokio::main` for its existing subcommands. The testbed commands are **synchronous** — they call into the sync `foundation_testbed` API directly. No `tokio::spawn` or async wrappers needed.

Subcommand tree:

```
ewe_platform testbed
├── start <os> [--headless | --headful]
├── stop <os>
├── build <os> [--project <path>]
├── exec <os> <cmd>
├── shell <os>
├── run <os> [--bin <path>]
├── screenshot <os> --out <file>
├── logs <os> [--follow | --errors] [--tail N] [--kind build|run]
├── doctor [<os>]
├── push <os> --from <src> --to <dst>
├── pull <os> --from <src> --to <dst>
├── package <os> --out <file>
├── resize-disk <os> --plus-gb <N>
├── ls
├── snapshot
│   ├── save <os> <name>
│   ├── load <os> <name>
│   ├── delete <os> <name>
│   └── list <os>
└── ui (headful only)
    ├── click <os> --x <N> --y <N>
    ├── type <os> --text <string>
    └── validate <os> --template <file>
```

### 6.2 UI Testing Automation (Headful Mode)

When a VM is running in headful mode (SPICE display), provide basic UI automation:

- **click**: Send mouse click at (x, y) coordinates via SSH PowerShell (Windows) or `xdotool` (Linux)
- **type**: Send keystrokes via SSH PowerShell `Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]` (Windows) or `xdotool type` (Linux)
- **validate**: Compare current screenshot against a template (golden image comparison)
  - Template files stored in project's `testbed/` directory
  - Compare via pixel diff with configurable tolerance (e.g., 95% match = pass)
  - Output diff image showing mismatch areas

**Note:** These commands require an active desktop session. In headless mode (VNC only), click/type still work via `xdotool`/PowerShell but visual validation requires the screenshot command to capture what's actually rendered.

### 6.3 README Documentation

`backends/foundation_testbed/README.md` with:

- Overview and quickstart
- Prerequisites (mise, KVM, pre-built images)
- All CLI commands with examples
- Profile configuration (`~/.config/foundation_testbed/config.toml`)
- Troubleshooting guide
- Architecture overview
- How to add custom VM profiles
- How to build and publish pre-baked images

### 6.4 mise + nushell Integration

`mise.toml` in `backends/foundation_testbed/`:

```toml
[tools]
qemu = "latest"
nu = "latest"
```

The README instructs users to run `mise install` before first use. The `doctor` command checks if `qemu-system-x86_64`, `qemu-img`, and `nu` are on PATH and suggests `mise install` if missing.

We do **not** require mise to be installed — if the user has QEMU and nushell from another source (system package manager, manual install), it works equally well. mise is just the recommended way.

Inside VMs, the bootstrap installs mise and nushell, then uses mise to manage all tool versions. The bootstrap `mise.toml` declares `nu = "latest"` so nushell is available as the execution shell for all VM-side commands.

## Implementation Phases

### Phase 1: CLI Wiring (Tasks 1-2)
1. Create `bin/platform/src/testbed/mod.rs` — clap subcommand registration, argument parsing, delegation to `foundation_testbed` API
2. Wire into `bin/platform/src/main.rs` — add `testbed::register(commander)` and dispatch

### Phase 2: UI Testing (Tasks 3-4)
3. Create `src/runner/ui.rs` — click, type via SSH (PowerShell for Windows, xdotool for Linux)
4. Create `src/runner/validate.rs` — golden image comparison, diff output

### Phase 3: Documentation (Task 5)
5. Write `backends/foundation_testbed/README.md` — comprehensive documentation with quickstart

## Success Criteria

- [ ] `ewe_platform testbed doctor` works from anywhere in the project
- [ ] `ewe_platform testbed start windows --headless` boots VM from the platform binary
- [ ] `ewe_platform testbed ui click windows --x 500 --y 300` sends a mouse click
- [ ] `ewe_platform testbed ui validate windows --template testbed/golden-windows.png` compares screenshot
- [ ] README covers all commands with examples
- [ ] `mise install` in `backends/foundation_testbed/` installs QEMU

## Verification Commands

```bash
# CLI integration
ewe_platform testbed doctor

# Build
cargo build -p foundation_testbed
cargo build -p platform

# Test
cargo test -p foundation_testbed
```

---

## Implementation Plan

### bin/platform Integration Pattern

The existing `bin/platform/src/main.rs` pattern:

```rust
// bin/platform/src/main.rs
mod testbed;

// In main():
commander = testbed::register(commander);

// In match:
Some(("testbed", arguments)) => testbed::run(arguments)?,
```

The `testbed` module exposes:
```rust
pub fn register(cmd: Command) -> Command { ... }
pub fn run(args: &ArgMatches) -> Result<(), BoxedError> { ... }
```

The `run` function is synchronous — it calls `foundation_testbed::start()`, `foundation_testbed::build()`, etc. directly. The `tokio::main` on `main()` doesn't interfere; sync calls from async context are fine (we're not blocking the runtime because we're the only "task" running).

### UI Automation Approach

**Windows (via nushell over SSH):**

```nu
# Click at (x, y) — nushell calls PowerShell through ^ for externals
^Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.Cursor]::Position = [System.Drawing.Point]::new(500, 300)
```

**Linux (via nushell over SSH):**

```nu
# Click at (x, y)
^xdotool mousemove 500 300 click 1

# Type text
^xdotool type --delay 50 "hello world"
```

The nushell wrapper provides a unified API: `nu -c "click 500 300"` works on both OSes, with the OS-specific implementation hidden behind a `runner::ui` module.

### Golden Image Comparison

```rust
fn compare_screenshot(actual: &Path, golden: &Path, tolerance: f32) -> Result<(), Diff> {
    // 1. Load both images (image crate)
    // 2. Resize actual to match golden dimensions if needed
    // 3. Pixel-by-pixel comparison
    // 4. Calculate match percentage
    // 5. If below tolerance, generate diff image highlighting mismatches
    // 6. Return Ok if above tolerance, Err(diff) otherwise
}
```

The `image` crate (`image = "0.25" in Cargo.toml) handles PNG loading and pixel access.

### Pre-built Image Sourcing Strategy

For the README and default config, we need pre-built qcow2 images:

1. **Short term**: Use quickemu's pre-built images or build one with Packer and host it
2. **Long term**: Publish to a registry (GitHub Releases, Cloudflare R2)

The `prebaked_url` field in profiles points to the download URL. Users can override with their own images via the config file.

Images are stored globally at `$HOME/.testbed/images/` so they're shared across projects. State lives at `$PWD/.testbed/state/` so each project tracks its own VM.

**Why pre-built matters:** With a raw ISO, the first boot requires a 20-40 minute unattended Windows installation. The tool can't SSH in until that completes. With a pre-built image, the OS is already installed — the first boot takes ~2 minutes to reach the desktop, then SSH is immediately available, then the tool bootstrap (VS Build Tools, etc.) runs.

The bootstrap flow with pre-built image:
```
Boot QEMU (2 min) → SSH ready → Bootstrap tools (10-15 min) → Ready to build
```

vs raw ISO:
```
Boot QEMU (30 sec) → Unattended install (20-40 min) → Reboot (2 min) → SSH ready → Bootstrap tools (10-15 min) → Ready to build
```
