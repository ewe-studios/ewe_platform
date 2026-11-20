---
description: "Restore llama.cpp vendoring into infrastructure_llama_bindings so its crate builds from its own tarball, then publish the workspace crates in dependency order"
status: "in-progress"
priority: "high"
created: 2026-07-23
author: "Main Agent"
metadata:
  version: "2.0"
  last_updated: 2026-11-20
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
builds_on: "specifications/completed/49-crates-io-publishing"
related_specs:
  - "specifications/completed/49-crates-io-publishing"
  - "specifications/60-agentic-reliability"
tasks:
  completed: 4
  uncompleted: 1
  total: 5
  completion_percentage: 80
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

## Measured state (2026-11-20)

The release half of this spec is done. The consumer half is **not**.

| Item | Measured |
|---|---|
| Vendored path | `infrastructure/llama-bindings/llama.cpp` is a tracked symlink (mode 120000) to `tools/llama.cpp` |
| Package contents | `cargo package --list` lists 1216 `llama.cpp` files, including `include/llama.h` and `src/llama.cpp` |
| Tarball size | 1229 files, 28.1 MiB unpacked, 5.6 MiB compressed. Under the 10 MiB default |
| Published versions | `infrastructure_llama_bindings` 0.0.5, `infrastructure_llama_cpp` 0.0.6, `foundation_ai` 0.0.9, `foundation_repl` 0.2.4, `foundation_core` 0.0.21 |
| 0.0.1 | yanked |
| llama.cpp | updated from `b12032` to `b12563` (commit `9fbe30c2a`) |
| Consumer build | fixed: the isolated tarball builds with no build env (R2, R3, R6) |
| Wrapper tests | 71 doc tests and 15 integration tests pass |
| Consumers | `cargo check -p foundation_ai` passes |

**The consumer build failed before this change.** Extract the published `.crate` to an empty
directory outside the repository and unset `LLAMA_DIR`, `TOOLS_DIR` and
`EMSDK_DIR`:

```bash
tar xzf target/package/infrastructure_llama_bindings-0.0.5.crate -C /tmp/pkg
cd /tmp/pkg/infrastructure_llama_bindings-0.0.5
env -u LLAMA_DIR -u TOOLS_DIR -u EMSDK_DIR cargo check --lib
```

Result:

```
error: failed to run custom build command for `infrastructure_llama_bindings v0.0.5`
thread 'main' panicked at build.rs:343:32:
  get TOOLS_DIR environment: NotPresent
```

The tarball holds the sources, but `build.rs` never uses them without the
environment:

- `build.rs:343` requires `TOOLS_DIR`, then stores it in `_tools_directory`,
  which nothing reads.
- `build.rs:346` requires `EMSDK_DIR` only to add `upstream/emscripten` to the
  CMake prefix path.
- `build.rs:352` requires `LLAMA_DIR` with no default.

The workspace hides this. `.cargo/config.toml` sets `TOOLS_DIR`, `LLAMA_DIR`
and `EMSDK_DIR` as repo-relative values, and mise exports absolute values. A
`cargo publish --dry-run` from the repository therefore passes. The CI check
`scripts/ci/package_workspace.py` runs `cargo package --no-verify`; its own
docstring says it does not build the tarball. Both checks miss the defect.

# Decisions

| # | Decision | Rationale |
|---|---|---|
| 1 | Keep a tracked symlink at `infrastructure/llama-bindings/llama.cpp` to `tools/llama.cpp` | Shipped. `cargo package` follows it for the `include` globs: the tarball holds 1216 real files. The original `git mv` plan is not needed |
| 2 | `LLAMA_DIR` defaults to `CARGO_MANIFEST_DIR/llama.cpp`, override still honoured | **Not implemented.** The default is the core missing piece |
| 3 | Yank `infrastructure_llama_cpp 0.0.1`, then publish fixed versions | Done: 0.0.1 is yanked. Later releases are 0.0.5 bindings and 0.0.6 llama_cpp |
| 4 | Tarball size decides nothing until measured | Measured: 5.6 MiB compressed, under the 10 MiB default |
| 5 | **Build the extracted tarball in an empty directory with no build env** | The acceptance test. A repo dry-run passes only because `.cargo/config.toml` and mise supply the variables. It missed the defect |

# Requirements

## R1 — llama.cpp is inside the bindings package

- **Shipped:** `infrastructure/llama-bindings/llama.cpp` is a tracked symlink to
  `tools/llama.cpp`.
- **Measured:** `cargo package --list` lists 1216 `llama.cpp` files, so the
  tarball holds real sources. `cargo package` follows the symlink for the
  `include` globs. The original `git mv` plan is not needed.
- Every script, Dockerfile, CI step and env var keeps one path. Do not add a
  second copy.

## R2 — `build.rs` works with no environment set

- `LLAMA_DIR` unset resolves to `CARGO_MANIFEST_DIR/llama.cpp`, which exists in
  the tarball. `LLAMA_DIR` set still wins.
- `TOOLS_DIR` is required today and stored in `_tools_directory`, which nothing
  reads. Remove the `expect`, or make it optional.
- `EMSDK_DIR` is required only to add `upstream/emscripten` to the CMake prefix
  path. Make it optional; add the path only when it is set.
- The `include` list is reconciled against what CMake reads. Anything CMake
  needs and `include` omits fails the tarball build.

## R3 — the package builds from its own tarball

- Extract the `.crate` outside the repository. Unset `LLAMA_DIR`, `TOOLS_DIR`
  and `EMSDK_DIR`. Then `cargo check --lib` succeeds:

  ```bash
  cargo package -p infrastructure_llama_bindings --allow-dirty
  rm -rf /tmp/pkg && mkdir -p /tmp/pkg
  tar xzf target/package/infrastructure_llama_bindings-*.crate -C /tmp/pkg
  cd /tmp/pkg/infrastructure_llama_bindings-*
  env -u LLAMA_DIR -u TOOLS_DIR -u EMSDK_DIR cargo check --lib
  ```

- Repeat for `infrastructure_llama_cpp` and for its shipped examples.
- `cargo package --list` is reviewed. The measured size is 5.6 MiB compressed,
  under crates.io's 10 MiB default.

## R4 — publish a fixed version

- The release half is done: 0.0.1 is yanked, bindings 0.0.5 and llama_cpp 0.0.6
  are live, and the others are live (see the measured table).
- Those live versions carry the consumer defect. After R2 and R3 pass, release
  a new version of both crates.
- Release-plz owns versions: do not bump by hand. Merge the fix, let the release
  PR bump, then let the publish job run. Record the fixed version here.

## R5 — CI catches a consumer build failure

- `scripts/ci/package_workspace.py` packages with `--no-verify` and does not
  build a tarball.
- Add one isolated check: extract one packaged crate to a temp dir outside the
  repository, unset the build env vars, and run `cargo check --lib`.
- A failure fails the check. This is the regression guard for R3.

## R6 — track the latest llama.cpp release

- `tools/llama.cpp` moves from `b12032` to the latest release tag `b12563`
  (commit `9fbe30c2a`).
- The wrapper migrates to the new API:
  - session save/load → `llama_state_save_file` / `llama_state_load_file`;
  - `llama_get_state_size` → `llama_state_get_size`;
  - `llama_copy_state_data` / `llama_set_state_data` → `llama_state_get_data` /
    `llama_state_set_data` (both take a size);
  - `llama_n_ctx_train` → `llama_model_n_ctx_train`;
  - `llama_token_bos` / `eos` / `nl` → `llama_vocab_bos` / `eos` / `nl`;
  - `llama_token_is_eog` → `llama_vocab_is_eog`;
  - `llama_token_get_attr` → `llama_vocab_get_attr`;
  - `llama_n_vocab` → `llama_vocab_n_tokens`;
  - `llama_n_embd` → `llama_model_n_embd`;
  - `llama_load_model_from_file` → `llama_model_load_from_file`;
  - `llama_new_context_with_model` → `llama_init_from_model`;
  - `llama_free_model` → `llama_model_free`;
  - `llama_sampler_init_grammar_lazy` → `llama_sampler_init_grammar_lazy_patterns`.
- The `include` list adds `**/*.in`, `**/*.cmake` and `LICENSE`. CMake needs
  `ggml-version.h.in`, `llama-version.h.in`, `common/parsers/sources.cmake`
  and the license file.
- Gate: `cargo check -p infrastructure_llama_cpp`,
  `cargo test -p infrastructure_llama_cpp`, `cargo check -p foundation_ai`,
  and the isolated tarball build.

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
- **A green local build proves nothing about a tarball.** Only a build of the
  extracted tarball exercises the packaged tree.
- **The repository supplies the build environment.** `.cargo/config.toml` sets
  `TOOLS_DIR`, `LLAMA_DIR` and `EMSDK_DIR`; mise exports them too. A dry run from
  the repository passes. A consumer has none of them. Unset them for the test.
- **The CI package check does not build.** `package_workspace.py` uses
  `cargo package --no-verify` by design. It cannot catch a `build.rs` that needs
  the repository. R5 adds the missing check.

# Tasks

- [x] **T1 — restore the vendoring and the package contents.** The symlink, the
      1216 packaged `llama.cpp` files and the 5.6 MiB tarball.
- [x] **T2 — publish and yank.** 0.0.1 yanked; bindings 0.0.5 and llama_cpp
      0.0.6 live.
- [x] **T3 — consumer build with no environment.** `build.rs` defaults,
      optional `EMSDK_DIR`, no `TOOLS_DIR`, and the isolated tarball build.
- [x] **T4 — update llama.cpp.** `b12563` with the wrapper API migration.
- [ ] **T5 — release the fixed versions.** release-plz publishes after the
      merge; record the live versions here.

# Out of scope

- Replacing the hand-written bindings with the published `llama-cpp-sys-2`.
- Publishing the ~36 crates from spec 49 that are unrelated to this dependency
  chain.
