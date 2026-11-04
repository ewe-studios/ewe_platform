# Upstream patch for gemm-common 0.19.0

Status: the upstream pull request is prepared on branch `fix-fma-without-avx2`
in the main session's clone of https://github.com/sarah-ek/gemm/. Link pending:
the issue and the pull request are not filed yet. When they exist, add their
links here and in `docs/bug-backlog.md` section 10.

The patch is the `gemm-common` part of the upstream change. It is the only
difference between this directory and the crates.io release `gemm-common`
0.19.0 (file `src/gemm.rs`). The same upstream change also touches `gemm-f16`;
that part is vendored in `patches/gemm-f16` (see the end of this file).

## Issue text

**Title:** `gemm-common` panics on x86-64 CPUs that report `fma` but not `avx2`

On x86-64, `init_gemm_fn` (and `init_gemm_cplx_fn`) choose the `fma` kernel
when the CPU reports the `fma` feature. That kernel uses the `V3` SIMD type
from `pulp`. `V3` needs both `fma` and `avx2`. When a CPU reports `fma` but not
`avx2`, `V3::try_new()` returns `None`, and the `.unwrap()` in the kernel
wrapper (`__inject_mod!`) panics. `gemm-f32` and every crate above it
(`candle-core`, and so on) then panic on the first matrix multiplication.

Real hardware with `fma` and no `avx2` is rare. Emulators are the common case:
Docker Desktop on Apple Silicon runs `linux/amd64` containers with QEMU or
Rosetta, and some builds report `fma` without `avx2`.

How to reproduce: run any `gemm::gemm` call for `f32` in a `linux/amd64`
container under Docker Desktop on an Apple Silicon Mac. The program panics at
`called Option::unwrap() on a None value`.

Fix: pick each x86 kernel by asking its SIMD type, not one CPU flag. The
dispatch checks `V3::try_new().is_some()` (and `V4::try_new().is_some()` for
`avx512f`), so the choice and the kernel's own `unwrap()` use the same check.
The `fma` kernel needs avx, avx2 and sse3..sse4.2 (and f16c with the `f16`
feature), so `fma` alone is not enough. The patch is below.

## Patch

```diff
--- a/src/gemm.rs
+++ b/src/gemm.rs
@@ -1011,11 +1011,15 @@ macro_rules! gemm_def {
         fn init_gemm_fn() -> GemmTy {
             #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
             {
+                // Pick a kernel by asking its SIMD type, not one CPU flag: the kernel
+                // `unwrap()`s `try_new()`, which needs every feature the type lists
+                // (e.g. `fma` alone is not enough; some CPUs and x86 emulators report
+                // FMA without AVX2).
                 #[cfg(feature = "x86-v4")]
-                if $crate::feature_detected!("avx512f") {
+                if $crate::simd::V4::try_new().is_some() {
                     return avx512f::gemm_basic;
                 }
-                if $crate::feature_detected!("fma") {
+                if $crate::simd::V3::try_new().is_some() {
                     fma::gemm_basic
                 } else {
                     scalar::gemm_basic
@@ -1124,11 +1128,12 @@ macro_rules! gemm_cplx_def {
         fn init_gemm_cplx_fn() -> GemmCplxTy {
             #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
             {
+                // See `init_gemm_fn`: ask the kernel's SIMD type, not one flag.
                 #[cfg(feature = "x86-v4")]
-                if $crate::feature_detected!("avx512f") {
+                if $crate::simd::V4::try_new().is_some() {
                     return avx512f_cplx::gemm_basic_cplx;
                 }
-                if $crate::feature_detected!("fma") {
+                if $crate::simd::V3::try_new().is_some() {
                     return fma_cplx::gemm_basic_cplx;
                 }
             }
```

The post-image of `src/gemm.rs` in the upstream commit has git blob `fd37141`,
the same as this vendored file.

## Why this stays a `[patch.crates-io]` entry

`gemm-common` is a transitive dependency of `candle-core`, so we cannot rename
it without a fork of candle. The root `Cargo.toml` patch applies only inside
this workspace. A user of the published `foundation_ai` gets the unpatched
crate. Remove `patches/gemm-common` and the `[patch.crates-io]` section when
a release of `gemm-common` with this fix is on crates.io and candle accepts it.
See `docs/bug-backlog.md` section 10.

## gemm-f16 (vendored in `patches/gemm-f16`)

The `gemm-f16` part of the same upstream change is applied in
`patches/gemm-f16/src/gemm.rs`: `init_gemm_fn` dispatches on
`gemm_common::simd::V3::try_new().is_some()` and `V4::try_new().is_some()`, the
same as gemm-common. The f16 `V3` also needs F16C. Post-image blob `e8200e9`
matches the upstream diff. `gemm-f16` 0.19.0 is in the workspace graph through
candle-core and `gemm`, so this patch is needed for the f16 path. The rest of the
crate is unchanged from the crates.io release. The upstream PR is part of the
same prepared branch, `fix-fma-without-avx2`; link pending, as above.
