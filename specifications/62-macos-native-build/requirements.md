---
description: "Make ewe_platform compile on macOS (Apple M5) and harden the cross-platform build toolchain so native `cargo build` works out of the box on macOS and Arch Linux"
status: "complete"
priority: "high"
created: 2026-10-17
author: "Main Agent"
metadata:
  version: "1.3"
  last_updated: 2026-10-18
  estimated_effort: "medium"
  tags:
    - macos
    - build
    - toolchain
    - kqueue
    - cranelift
    - native
  stack_files: [rust.md]
  skills: []
  tools: []
has_features: true
has_fundamentals: false
related_specs: []
tasks:
  completed: 10
  uncompleted: 0
  total: 10
  completion_percentage: 100%
---

# Overview

Make the workspace build on the Apple M5 MacBook Air, and keep it building on
Arch Linux. The trigger was a bindgen failure
(`wrapper.h:1:10: fatal error: 'llama.h' file not found`) that turned out to be
the first of several stacked, macOS-specific breakages. Each breakage is one
feature in this spec. All ten findings are now fixed. F07 (Windows/shell
selector parity) grew beyond its original `new()`-only scope: installing the
mingw-w64 + `x86_64-pc-windows-gnu` toolchain made the Windows selector
compilable for the first time, which surfaced and fixed 32 latent compile
errors. F08 (Android NDK) is done via a build-time wrapper + installer script.
F10 clears the last warnings: workspace-wide lint inheritance, optional `vfs`
dependencies, and the macOS kqueue handler's missing bus delivery.

# Problem

The workspace was developed and only ever built on Arch Linux. Porting to macOS
surfaced, in order:

1. `tools/llama.cpp` is a git submodule that was never initialised on this
   machine → `llama.h` missing (F02).
2. A stray `tools/depot_tools` gitlink with no `.gitmodules` entry aborts a bare
   `git submodule update` (F03).
3. `clang-sys 1.8.x` does not search `/opt/homebrew` on Apple Silicon → libclang
   not found at bindgen time (F04).
4. The `dev` profile uses the cranelift codegen backend, but the toolchain does
   not install the `rustc-codegen-cranelift` component (F05).
5. `foundation_nativeapis` has a hand-written kqueue selector that was never
   compiled on macOS and fails with 16 errors (F06).
6. The Windows and "other-OS" selectors share the same `new_with_registry`-only
   shape as kqueue and failed the same way once the Windows target was compiled
   (F07).
7. Android cross-compile linker paths are hard-coded to a Linux home dir (F08).

# Completed findings (this effort)

Already fixed and committed — retained here for lineage. Detail lives in git
history; `mise run setup` now covers F02/F04 going forward.

| ID | Finding | Fix |
|----|---------|-----|
| F01-per-os-setup | `mise run setup` assumed Arch Linux only | New `scripts/setup/{common,macos,archlinux}.sh`; `[tasks.setup]` dispatches on `uname -s` |
| F02-llama-cpp-submodule | `llama.h not found` — submodule never initialised | `git submodule update --init --recursive tools/llama.cpp` (and `init_submodules` in `common.sh`) |
| F03-depot-tools-gitlink | stray gitlink aborted `git submodule update` | `git rm --cached tools/depot_tools` + `.gitignore` entry; submodule updates are path-scoped |
| F04-libclang-apple-silicon | clang-sys won't find libclang under `/opt/homebrew` | `brew link llvm --force` + `LIBCLANG_PATH="$(brew --prefix llvm)/lib"` persisted to `~/.zshrc` |

# Feature Index (remaining work)

| ID | Title | Status |
|----|-------|--------|
| [F05-cranelift-toolchain](features/F05-cranelift-toolchain/feature.md) | Install `rustc-codegen-cranelift` on all platforms; reconcile the duplicated `dev` profile | complete |
| [F06-kqueue-selector-compile](features/F06-kqueue-selector-compile/feature.md) | Fix the 16 `foundation_nativeapis` kqueue compile errors | complete |
| [F07-selector-new-parity](features/F07-selector-new-parity/feature.md) | Cross-compile the Windows selector: `Selector::new()` parity, unix gating, HANDLE fixes | complete |
| [F08-android-ndk-linkers](features/F08-android-ndk-linkers/feature.md) | Make Android NDK linker paths OS-relative | complete |
| [F09-build-verification-loop](features/F09-build-verification-loop/feature.md) | Rebuild `foundation_ai` + `answerme-agent`; turn each new breakage into a feature | complete |
| [F10-warning-hygiene](features/F10-warning-hygiene/feature.md) | Fix lint inheritance, optional `vfs` deps, and macOS signal delivery at their source | complete |

# High-level architecture

`foundation_nativeapis` is the platform-abstraction crate. Its `native/poll`
module is an `mio`-style I/O readiness layer with one selector backend per
platform, selected by `cfg(target_os)`:

- **Linux** — `sys::unix::selector::dispatch::Selector` (runtime epoll/io_uring
  ladder) or `epoll::Selector` when `uring` is off.
- **macOS/BSD** — `sys::unix::selector::kqueue::Selector`.
- **Windows** — `sys::windows::Selector`.
- **other** — `sys::shell::Selector`.

`Poll::new()` calls `sys::Selector::new()` (see `native/poll/mod.rs:126`). The
epoll selector ships `new()`; the kqueue/windows/shell selectors previously
shipped only `new_with_registry()`. F06 fixed kqueue on macOS, and F07 added
`new()` to windows/shell and validated the Windows backend via
`cargo check --target x86_64-pc-windows-gnu` (epoll remains the only backend
exercised in CI).

Crate feature flags (`foundation_nativeapis/Cargo.toml`):

- `default = ["native", "signal"]`, `native = ["poll", "fd"]`, `signal = ["poll"]`.
- `poll` pulls in `net` and the platform selector; `watcher` is always compiled
  and selects `unix`/`linux`/`windows` internally by `cfg(target_os)`.

A targeted scan confirmed the Linux-only `libc` symbols (`epoll_*`, `eventfd`,
`inotify_*`) all sit in already-`cfg`-gated files (`epoll.rs`, `uring*.rs`,
`probe.rs`, `watcher/linux.rs`, `waker/eventfd.rs`). So F06's kqueue errors are
expected to be the **only** `foundation_nativeapis` compile blocker; any further
breakages will surface in downstream crates (`foundation_ai`, `answerme-agent`,
and their dependents), which is exactly what F09 exists to enumerate.

# Success criteria

1. `cargo build -p foundation_ai --profile uat` succeeds on macOS (aarch64).
2. `cargo build -p answerme-agent --profile uat` succeeds on macOS (aarch64).
3. `cargo build -p foundation_ai` (default `dev` profile) succeeds on macOS after
   the cranelift component is installed (F05).
4. `mise run setup` is idempotent on macOS and Arch Linux and leaves the
   toolchain with `rustc-codegen-cranelift` installed.
5. `foundation_nativeapis` compiles on macOS with zero errors (F06), the
   `Selector::new()` contract is symmetric across epoll/kqueue/windows/shell,
   and `cargo check --target x86_64-pc-windows-gnu` passes with zero errors
   (F07).
6. Every remaining breakage discovered during F09 is either fixed or captured as
   a new feature in this spec with a plan — nothing is left as an un-specified
   stub.
7. `foundation_nativeapis` compiles warning-clean on macOS (default and `vfs`)
   and Windows, and every workspace member declares a `[lints]` table (F10).

# Out of scope

- Replacing the hand-written kqueue/epoll selectors with a published crate
  (`mio`, `polling`). The current design is retained; only correctness/compilation
  is addressed.
- Cross-compiling or publishing for iOS/Android in this spec (F08 only makes the
  NDK path OS-relative; it does not add new targets).
- The pre-existing soundness caveat in `Events::as_mut_ptr_and_cap` (a `*mut
  Event` buffer filled as `*mut libc::kevent`). It compiles and matches the
  single-field `Event { event: libc::kevent }` layout in practice; changing it is
  tracked as a separate concern, not this spec.
