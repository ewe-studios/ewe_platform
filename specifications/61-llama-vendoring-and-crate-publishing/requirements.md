---
description: "Restore llama.cpp vendoring into infrastructure_llama_bindings so its crate builds from its own tarball, then publish the workspace crates in dependency order"
status: "in-progress"
priority: "high"
created: 2026-07-23
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-07-23
  estimated_effort: "medium"
  tags:
    - publishing
    - crates-io
    - ffi
    - llama-cpp
    - build-scripts
  skills: []
  tools: []
has_features: false
has_fundamentals: false
builds_on: "49-crates-io-publishing"
related_specs:
  - "49-crates-io-publishing"
  - "60-agentic-reliability"
---

# Overview

`cargo publish` cannot build `infrastructure_llama_bindings` from the tarball it
produces, because the C++ sources it compiles live outside the package. Every
crate above it inherits the problem. This spec restores the vendoring the crate
was originally designed around, proves it with `cargo publish --dry-run`, and
then publishes the changed workspace crates in dependency order.

# Problem

`build.rs` takes the llama.cpp location from the environment:

```rust
// infrastructure/llama-bindings/build.rs:228
&env::var("LLAMA_DIR").expect("get LLAMA_DIR environment"),
```

Nothing sets `LLAMA_DIR` during `cargo publish`, and the sources are not in the
package, so verification fails:

```
error: failed to run custom build command for `infrastructure_llama_bindings v0.0.2`
Caused by:
  cargo:rerun-if-changed=/home/.../ewe_platform/tools/llama.cpp/CMakePresets.json
```

The crate was **built to vendor these sources**. `Cargo.toml` still carries an
`include = [...]` list full of entries like `/llama.cpp/ggml/include/*.h` and
`/llama.cpp/include/llama.h`, which only resolve if llama.cpp sits at
`infrastructure/llama-bindings/llama.cpp/`. It was relocated to
`tools/llama.cpp` and the include list was left behind. The two halves of one
design drifted apart; this is a restoration, not a redesign.

## Why the obvious workarounds do not apply

- **Symlinks.** `cargo package` does not follow them, and a symlink that
  survived would point outside the extracted tarball on a consumer's machine.
- **Git submodules.** Same failure — submodule contents are not in the tarball.
  (`tools/dawn` and `artefacts/models/whisper.cpp` are submodules; llama.cpp is
  a plain directory.)

A published crate must be self-contained inside its own tarball. Any mechanism
that reaches outside the package directory fails identically.

## Consequence already shipped

`infrastructure_llama_cpp 0.0.1` is on crates.io and, carrying this same defect,
almost certainly does not build for anyone who depends on it. It was published
past verification.

# Decisions

| # | Decision | Rationale |
|---|---|---|
| 1 | Vendor llama.cpp at `infrastructure/llama-bindings/llama.cpp/` | Restores what `include = [...]` already expects |
| 2 | `LLAMA_DIR` defaults to `CARGO_MANIFEST_DIR/llama.cpp`, override still honoured | Publishing works with no environment; local builds keep their escape hatch |
| 3 | **Yank `infrastructure_llama_cpp 0.0.1`, publish `0.0.2`** | 0.0.1 cannot build for consumers; leaving it is worse than removing it |
| 4 | Tarball size decides nothing until measured | Size is governed by the `include` list, not the 156 MB on disk. Measure, do not assume |
| 5 | `cargo publish --dry-run` is the acceptance test | A local `cargo build` passing proves nothing here — it was passing throughout |

# Requirements

## R1 — llama.cpp vendored inside the bindings crate

- `tools/llama.cpp` moves to `infrastructure/llama-bindings/llama.cpp` via
  `git mv`, preserving history.
- Every script, Dockerfile, CI step and env var referencing the old path is
  updated — including whatever copies `llama-server`, and anything exporting
  `LLAMA_DIR` or `TOOLS_DIR`.

## R2 — build.rs works with no environment set

- `LLAMA_DIR` unset resolves to the vendored copy.
- `LLAMA_DIR` set still wins, so local out-of-tree builds keep working.
- The `include` list in `Cargo.toml` is reconciled against what CMake actually
  reads; anything CMake needs and `include` omits fails the dry run.

## R3 — the package builds from its own tarball

- `cargo publish --dry-run -p infrastructure_llama_bindings` succeeds.
- `cargo publish --dry-run -p infrastructure_llama_cpp` succeeds.
- `cargo package --list` is reviewed and the tarball size recorded here. If it
  exceeds crates.io's 10 MiB default, prune further or request an increase —
  decided on the measurement, not in advance.

## R4 — publish in dependency order

1. `foundation_repl` 0.2.0 → 0.2.1 — additive API only (`ReplTheme::named`,
   `from_env`, `from_env_or`); no cascade.
2. `foundation_core` 0.0.4 → 0.0.5 — valtron log-level changes. **Cascades**:
   every dependent's `version =` spec must follow.
3. Infrastructure crates — yank `infrastructure_llama_cpp 0.0.1`, publish
   `infrastructure_llama_bindings` then `infrastructure_llama_cpp` 0.0.2.
4. `foundation_ai` — first publish, so every path dependency must already be on
   crates.io.
5. Remaining dependents — bump and commit first, then publish.

Version bumps and their cascade land as their own commit **before** any publish,
so a failed publish never leaves the tree half-bumped.

# Traps this spec exists to avoid

Each of these has already cost time in this workspace:

- **Multi-line dependency tables hide stale pins.** `[dependencies.foo]` with
  `version` on its own line is invisible to a grep for `foo = { version`. Two
  `foundation_errstacks` pins were missed this way and broke workspace
  resolution.
- **A stale lockfile hides an impossible manifest.** `getrandom = { version =
  "0.3", features = ["js"] }` could never resolve — `js` was 0.2's name — but
  nothing failed until an index refresh forced a re-resolve. Re-resolve
  deliberately before publishing.
- **`--no-verify` converts a build failure into a broken release.** That is how
  0.0.1 shipped. Do not reach for it.
- **A green local build proves nothing about a tarball.** Only `--dry-run`
  exercises the packaged tree.

# Out of scope

- Replacing the hand-written bindings with the published `llama-cpp-sys-2`.
- Publishing the ~36 crates from spec 49 that are unrelated to this dependency
  chain.
