# Upstream patch for esaxx-rs 0.1.10

Status: not yet filed upstream. File it before the patch is removed.

- Upstream: https://github.com/Narsil/esaxx-rs

The patch removes the two `.static_crt(true)` calls from `build.rs`. It is the
only difference between this directory and the crates.io release `esaxx-rs`
0.1.10, apart from the removed example, benchmark and dev-dependency entries,
which the patch does not build.

## Issue text

**Title:** `esaxx-rs` forces the static MSVC runtime, so a binary that uses the
dynamic runtime fails to link

`build.rs` calls `cc::Build::static_crt(true)` on every platform. On
`x86_64-pc-windows-msvc` that passes `/MT`, so the objects of `src/esaxx.cpp`
use `libcmt`/`libcpmt`. `cc` defaults to the target's `crt-static` feature, and
Rust's standard library uses the dynamic runtime (`/MD`). A link that mixes
both runtimes fails with `LNK2005` on `std::` symbols defined in both
`msvcprt.lib` and `libcpmt.lib`, and with `LNK1169`.

This workspace hits it in the `foundation_agent_rpc` gguf test: `tokenizers`
pulls `esaxx-rs`, and the same binary links llama.cpp, which uses the dynamic
runtime. Issue #772 tracks the failure.

**Fix:** remove the `static_crt` calls. `cc` then follows the target, which is
what every other C and C++ object in the link does.
