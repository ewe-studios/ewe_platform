# Upstream patch for gemm-common 0.19.0

Send this to https://github.com/sarah-ek/gemm/ (issue, then pull request).
The patch is two lines. It is the only difference between this directory and
the crates.io release `gemm-common` 0.19.0 (file `src/gemm.rs`).

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

Fix: check `avx2` together with `fma`, so the code falls back to the `scalar`
kernel (the real `fma`-only path has no kernel). The patch is below.

## Patch

```diff
--- a/src/gemm.rs
+++ b/src/gemm.rs
@@ -1015,7 +1015,10 @@
                 if $crate::feature_detected!("avx512f") {
                     return avx512f::gemm_basic;
                 }
-                if $crate::feature_detected!("fma") {
+                // The `fma` kernel uses the V3 SIMD type. That type needs AVX2.
+                // Check AVX2 too. If AVX2 is missing, `V3::try_new()` returns
+                // `None` and the unwrap in `__inject_mod!` panics.
+                if $crate::feature_detected!("fma") && $crate::feature_detected!("avx2") {
                     fma::gemm_basic
                 } else {
                     scalar::gemm_basic
@@ -1124,7 +1127,8 @@
                 if $crate::feature_detected!("avx512f") {
                     return avx512f_cplx::gemm_basic_cplx;
                 }
-                if $crate::feature_detected!("fma") {
+                // See `init_gemm_fn`: the `fma_cplx` kernel needs AVX2 too.
+                if $crate::feature_detected!("fma") && $crate::feature_detected!("avx2") {
                     return fma_cplx::gemm_basic_cplx;
                 }
             }
```

## Why this stays a `[patch.crates-io]` entry

`gemm-common` is a transitive dependency of `candle-core`, so we cannot rename
it without a fork of candle. The root `Cargo.toml` patch applies only inside
this workspace. A user of the published `foundation_ai` gets the unpatched
crate. Remove `patches/gemm-common` and the `[patch.crates-io]` section when
a release of `gemm-common` with this fix is on crates.io and candle accepts it.
See `docs/bug-backlog.md` section 10.
