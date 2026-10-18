# Learnings — 62: macOS native build

Non-obvious facts discovered while diagnosing this port. Future agents should
read these rather than re-derive them.

## Root causes

1. **`llama.h not found` was an uninitialised submodule, not an M5 problem.**
   `tools/llama.cpp` is a gitlink in the index but was never
   `git submodule update --init` on this machine, so bindgen found nothing.
   The crate already supported Apple Silicon + Metal; nothing platform-specific
   was wrong.

2. **A stray gitlink with no `.gitmodules` entry aborts `git submodule update`.**
   `tools/depot_tools` was a gitlink with no mapping, so a bare recursive
   submodule update died with `fatal: no submodule mapping found`. Path-scoping
   (`git submodule update --init <path>`) sidesteps it, but the gitlink itself
   was removed.

3. **clang-sys 1.8.x does not search `/opt/homebrew`.** Its macOS search list is
   `/usr/local/opt/llvm*/lib`, the CLT path, and the Xcode path — all Intel or
   Apple-owned prefixes. On Apple Silicon (brew at `/opt/homebrew`), libclang is
   only found if `LIBCLANG_PATH` points at the keg. `brew link llvm --force` makes
   the tools visible; `LIBCLANG_PATH` makes `clang-sys`/bindgen find the `.dylib`.

## `foundation_nativeapis` kqueue specifics

4. **`EV_SET` is a C macro, not a libc function.** There is no `libc::EV_SET`.
   The `libc::kevent` fields are assigned directly: `ident: usize (uintptr_t)`,
   `filter: i16`, `flags: u16`, `fflags: u32`, `data: isize (intptr_t)`,
   `udata: *mut c_void`.

5. **`libc::EVFILT_READ as _` is ambiguous.** `serde_json` implements
   `PartialEq<serde_json::Value> for i16`, so an untyped `as _` on the RHS of
   `==` yields two candidate `PartialEq` impls and E0283. Use an explicit
   `as i16`.

6. **`Poll::new()` calls `sys::Selector::new()`, but kqueue/windows/shell only
   define `new_with_registry()`.** Only `epoll::Selector` (and the Linux dispatch
   selector) have `new()`. The non-Linux selectors have never been compiled in CI.

7. **The kqueue selector was written but never built.** It failed on the first
   macOS compile with 16 errors across imports, `EV_SET`, `as _`, and the missing
   `new()`. This is the pattern to expect elsewhere: Linux-only CI means non-Linux
   backends are latent-broken until someone builds them.

## Toolchain

8. **The `dev` profile needs a component the toolchain does not install.**
   `[profile.dev] codegen-backend = "cranelift"` (present in both `Cargo.toml`
   and `.cargo/config.toml`) requires `rustc-codegen-cranelift`, which is absent
   from `rust-toolchain.toml`'s `components`. The `uat` profile
   (`codegen-backend = "llvm"`) is the working fallback until F05 lands.

9. **`[profile.dev]` is duplicated** in root `Cargo.toml` and `.cargo/config.toml`.
   Reconcile to a single source of truth in F05.

10. **Cranelift's aarch64 inline-asm assembler does not support `fullfp16`.**
    `foundation_ai`'s candle deps (`gemm-f16`/`gemm-common`) emit half-precision
    SIMD asm (`fadd v0.8h`, `fmla v0.8h`, `fmul v0.8h`) behind
    `#[target_feature(enable = "fp16,neon")]`. Under an all-cranelift `dev`
    build rustc fails with `instruction requires: fullfp16`. It is **not** fixed
    by `-C target-feature=+fp16,+fhm` or `-C target-cpu=native` — verified via
    `cargo build -v` that the flag reaches rustc, so this is a genuine cranelift
    limitation, not a config miss.

11. **`dev`/cranelift is a fast-iteration convenience, not a build bar.** The
    workspace crates compile under cranelift, but crates whose deps contain the
    fullfp16 asm (`foundation_ai` et al.) do not. The resolution is not backend
    forcing — it is to build those with `--profile uat` (llvm), the real build
    path. `[profile.dev]` stays `codegen-backend = "cranelift"` with no
    per-package override; profiles are selected, never overridden.

12. **The NDK linker path has two OS-specific parts, not one.** The obvious
    blocker was `/home/darkvoid` (a Linux home dir), but the deeper one is the
    host prebuilt tag: the NDK ships `linux-x86_64` tools on Linux and
    `darwin-x86_64` on macOS, so a single absolute linker path can never be
    correct for both. Fix: a repo-relative wrapper (`scripts/android-ndk/*`)
    that resolves the NDK + host tag at build time. Cargo resolves a relative
    `linker` against the directory of `.cargo/config.toml`, not the CWD
    (verified empirically), so `linker = "scripts/android-ndk/aarch64-linux-android"`
    works from anywhere in the tree. Also: 32-bit ARM is `armv7-linux-androideabi`
    in Rust but `armv7a-linux-androideabi` in the NDK tool name.

## Windows cross-compile (F07)

13. **`HANDLE` is a pointer, not an integer.** In windows-sys 0.59,
    `Win32::Foundation::HANDLE` is `*mut core::ffi::c_void` and
    `INVALID_HANDLE_VALUE = -1i32 as _`. Code written as `handle == 0`,
    `event.0`, or `Some(self.iocp)` fails to compile (or is wrong) — use
    `is_null()`, `INVALID_HANDLE_VALUE`, and pointer casts. A `HANDLE` is also
    `!Send`/`!Sync`, so any type holding one needs `unsafe impl Send/Sync`
    (it's a thread-safe kernel handle). Windows-sys module placement is not
    obvious: `WAIT_OBJECT_0` is in `Win32::Foundation`, `CreateIoCompletionPort`
    is in `Win32::System::IO`, and `SetConsoleCtrlHandler` + `CTRL_*_EVENT` are
    in `Win32::System::Console` (gated behind the `Win32_System_Console` feature).

14. **`std::io::Error` is neither `Clone` nor `Copy`**, so a
    `#[derive(Clone, Copy)]` struct with an `Option<std::io::Error>` field fails
    (E0204/E0277). `Events::iter()` uses `.copied()` (`events.rs:35`), which
    forces `Event: Copy` — store `Option<std::io::ErrorKind>` and reconstruct
    the `io::Error` in `error()` instead.

15. **The `new_with_registry()`-only selectors were the visible symptom; the
    real cost was that nothing after Linux had ever been compiled.** Once
    `cargo check --target x86_64-pc-windows-gnu` was enabled (mingw-w64 +
    `rustup target add x86_64-pc-windows-gnu`), the Windows selector/signal code
    surfaced 32 errors: unix-only modules (`native::net`, `native::fd`,
    `valtron::native`, `SourceFd`) that were feature-gated but never
    `cfg(unix)`-gated, plus the HANDLE-as-integer bugs.

16. **The Windows console handler must both signal its event and deliver to the
    bus.** The Linux `signal_handler` writes the eventfd *and* calls
    `deliver_signal(kind)`; the Windows `console_handler` originally did neither
    (its `GLOBAL_EVENT` atomic was never written, and `kind` was computed then
    dropped). `SetConsoleCtrlHandler` runs the handler on its own thread, so
    `deliver_signal` (which heap-allocates a `SignalEvent`) is safe there —
    unlike the async-signal context on Linux.

17. **`cargo check --target x86_64-pc-windows-gnu` needs only the std target;
    `cargo build` needs mingw-w64.** `check` type/lifetime-checks without
    invoking the linker, so it catches the HANDLE/unix-gating bugs without a
    full Windows link. That makes a macOS-hosted `check` a cheap, effective
    validation gate for the Windows backend.

## Warning hygiene (F10)

18. **`missing_lints_inheritance` fires for members with *no* `[lints]` table at
    all — not for members with overrides.** `foundation_compact` declares
    `[lints.rust]`/`[lints.clippy]` without `workspace = true` and does not
    warn: it still declares lints, so it is not "missing". The warning means
    "this member has no lints config, so it never inherits `[workspace.lints]`".
    The fix is any lints table — normally `[lints] workspace = true`. Enumerate
    members with `cargo metadata --no-deps` + `select(.source == null)` rather
    than grepping for a bare `[lints]`: a `grep '^\[lints\]'` misses
    `[lints.rust]`/`[lints.clippy]` and false-positives those members.

19. **Blanket `[lints] workspace = true` and `[lints.rust]`/`[lints.clippy]`
    sub-tables are mutually exclusive.** Appending `[lints] workspace = true` to
    a manifest that already has `[lints.rust]`/`[lints.clippy]` fails with
    "cannot override `workspace.lints` in `lints`". A member that needs to
    override a specific lint (e.g. `foundation_compact`'s `unexpected_cfgs`
    `check-cfg` for the `getrandom_*` cfgs) keeps its own `[lints.rust]`/
    `[lints.clippy]` table and does not inherit — that is valid and warning-free.
    (Per-group `workspace = true` inside `[lints.rust]`/`[lints.clippy]` is the
    alternate "inherit + override" form.)

20. **A dependency used only behind a feature must be `optional = true` + `dep:`.
    `async-trait`/`foundation_errstacks` were non-optional but only used by the
    `vfs` modules, so the `unused_dependencies` lint fired on every default
    check. Fix: `optional = true` on each dep and `vfs = ["dep:async-trait",
    "dep:foundation_errstacks"]` — the feature (and every `vfs-*` feature that
    implies it) still pulls them in, the default set no longer declares them
    unused.

21. **macOS kqueue consumes signals into the kernel — there is no user-space
    handler to deliver them.** Unlike Linux (handler writes eventfd +
    `deliver_signal`) and Windows (`SetConsoleCtrlHandler` thread), the macOS
    `SignalHandle` only ever waited on the kqueue and dropped the event, so
    `deliver_signal`/`deliver` were dead code. The `is_ready` branch must read
    the kevent and translate it back: `ev.filter == libc::EVFILT_SIGNAL as i16`
    (the `as i16` is required — same E0283 trap as `EVFILT_READ`), then map
    `ev.ident` (the signal number) to `SignalKind` and call
    `crate::signal::deliver_signal(kind)`. This closes the loop on macOS.
