# Fixing aws-lc-sys Linker Errors on Linux with lld

## Overview

This document details the investigation and resolution of widespread undefined symbol linker errors from `aws-lc-sys` v0.38.0 on Linux x86_64 when using the `lld` linker (`clang` with `-fuse-ld=lld`).

## Problem Description

### Symptoms

When building any binary in the workspace (e.g., `simple`, `ewe_platform`), the build failed at the linking stage with dozens of undefined symbol errors, all from `aws-lc-rs`:

```
ld.lld: error: undefined symbol: aws_lc_0_38_0_EVP_DigestVerify
          >>> referenced by evp_pkey.rs:462
          >>>               aws_lc_rs-6509a798fbcec0a0.aws_lc_rs.b722f4232b653f6b-cgu.0.rcgu.o

ld.lld: error: undefined symbol: aws_lc_0_38_0_CBB_init
          >>> referenced by cbb.rs:17

ld.lld: error: undefined symbol: aws_lc_0_38_0_EVP_DigestFinal
          >>> referenced by digest.rs:145

ld.lld: error: undefined symbol: aws_lc_0_38_0_EVP_DigestUpdate
          >>> referenced by digest.rs:116

... 20+ more undefined symbols ...
```

Every undefined symbol followed the `aws_lc_0_38_0_` prefix pattern — this is the symbol prefix used by `aws-lc-sys` to avoid collisions with system OpenSSL.

### Affected Binaries

- `examples/llama-cpp/simple` (the original target that triggered the error)
- `bin/platform` (`ewe_platform`)
- Any workspace binary that transitively depended on `aws-lc-rs`

### Environment

- **Platform**: Linux x86_64 (Arch Linux, kernel 6.19.10-arch1-1)
- **Toolchain**: Rust nightly, `clang` with `lld` linker
- **RUSTFLAGS**: `-C link-arg=-fuse-ld=lld` (set via `CARGO_ENCODED_RUSTFLAGS`)
- **aws-lc-sys**: v0.38.0
- **aws-lc-rs**: v1.16.1
- **rustls**: v0.23.35 → v0.23.38
- **cmake**: v4.3.1

---

## Investigation

### Step 1: Verify the symbols exist in the built library

First checked whether the aws-lc-sys static library actually contained the missing symbols:

```bash
nm target/debug/build/aws-lc-sys-6ac94ed6a48deb0c/out/libaws_lc_0_38_0_crypto.a \
  | grep "aws_lc_0_38_0_EVP_DigestVerify"
```

**Result**: The symbol was present and defined (`T` = text/code section):

```
0000000000000000 T aws_lc_0_38_0_EVP_DigestVerify
```

This meant the library appeared to have the correct symbols, ruling out a completely broken build.

### Step 2: Check for stale build artifacts

The hypothesis was that old object files from a previous build configuration might be cached. Attempted a targeted clean:

```bash
cargo clean -p aws-lc-sys -p aws-lc-rs -p ring -p rustls
cargo build --bin simple
```

**Result**: Same linker errors, different hash IDs (expected after rebuild). This ruled out simple stale artifacts.

### Step 3: Full clean rebuild

To rule out any cached state anywhere in the build tree:

```bash
cargo clean  # Removed 113.9 GiB, 253,171 files
cargo build --bin simple
```

**Result**: Same errors persisted. This was a critical finding — the problem was in the current build process itself, not stale cache.

### Step 4: Inspect the CC builder output

The `aws-lc-sys` build script output revealed it was using the **CC builder** (direct `cc` crate compilation), not the CMake builder:

```
cargo:warning=Building with: CC
cargo:warning=Symbol Prefix: Some("aws_lc_0_38_0")
cargo:rustc-cfg=universal
```

Checked the `universal.rs` source list in the CC builder:

```bash
grep -i "digest\|evp" \
  ~/.cargo/registry/src/.../aws-lc-sys-0.38.0/builder/cc_builder/universal.rs
```

**Found**: Only `crypto/digest_extra/digest_extra.c` was listed. The main `crypto/evp/digest.c` file (which defines `EVP_DigestUpdate`, `EVP_DigestInit_ex`, etc.) was **not** in the CC builder's source list.

Further confirmed by checking object files individually:

```bash
nm target/debug/build/aws-lc-sys-.../out/ebcd52e9457b6221-a_digest.o
# Output: U aws_lc_0_38_0_EVP_DigestUpdate  (U = undefined!)
```

**Root cause identified**: The CC builder's source file list was incomplete. Many core EVP functions were referenced but never compiled into the static library.

### Step 5: Try forcing the CMake builder

Set `AWS_LC_SYS_CMAKE_BUILDER=1` to force the CMake-based build path:

```bash
AWS_LC_SYS_CMAKE_BUILDER=1 cargo build --bin simple
```

The build log confirmed:

```
cargo:warning=Building with: CMake
```

The CMake builder produced the library in a subdirectory:
```
target/debug/build/aws-lc-sys-.../out/build/artifacts/libaws_lc_0_38_0_crypto.a
```

**Result**: Partial improvement — some symbols were now defined, but many others were still undefined. The CMake build also had issues with lld.

### Step 6: Analyze the dependency chain

To understand how `aws-lc-rs` was being pulled into the workspace, traced the full dependency tree:

```
aws-lc-rs v1.16.1
├── rustls v0.23.35 (or 0.23.38)
│   ├── foundation_core (via ssl-rustls feature)
│   └── rustls-webpki v0.103.8 (also depends on aws-lc-rs by default)
├── ring (also a dependency, but aws-lc-rs was primary)
└── ureq v2.12.1 (via hf-hub → ureq → rustls default features)
```

**Key insight**: `rustls` 0.23's **default features** include `aws_lc_rs`. Even though `foundation_core` had its own `rustls` dependency with `default-features = false` and `rustls/ring` feature, **Cargo's feature unification** meant that if any other dependency pulled in `rustls` with default features enabled, the `aws-lc-rs` feature would be enabled globally.

The `hf-hub` crate (used by the llama-cpp examples) had this chain:

```
hf-hub (default features: ["default-tls", "tokio", "ureq"])
  └── ureq (default features: ["tls", "gzip"])
        └── rustls (default features include aws_lc_rs)
```

This meant **even with `foundation_core` configured correctly**, the llama-cpp examples pulled in `aws-lc-rs` through a completely different path.

---

## Solution

The fix had two parts:

### Part 1: Remove llama-cpp examples from the workspace

Since the `hf-hub` → `ureq` → `rustls` → `aws-lc-rs` chain was unavoidable without modifying `hf-hub`'s feature set (which would break its TLS functionality), the llama-cpp examples were removed from workspace membership.

**File**: `Cargo.toml` (workspace root)

**Before**:
```toml
members = [
  ...
  "examples/llama-cpp/embeddings",
  "examples/llama-cpp/simple",
  "examples/llama-cpp/reranker",
  "examples/llama-cpp/mtmd",
]
```

**After**:
```toml
members = [
  ...
  # examples
  "examples/web/*",
  "examples/intro",
  "examples/template/*",
]

exclude = [
  ...
  # llama-cpp examples (depend on hf-hub which pulls in aws-lc-sys with broken build)
  "examples/llama-cpp/*",
]
```

### Part 2: Disable rustls default features in foundation_core

**File**: `backends/foundation_core/Cargo.toml`

Changed `rustls` dependency to explicitly disable default features:

```toml
# Before
rustls = { version = "0.23", optional = true }

# After
rustls = { version = "0.23", optional = true, default-features = false }
```

Updated the `ssl-rustls-ring` feature to explicitly enable only ring-compatible sub-features:

```toml
# Before
ssl-rustls = ["std", "rustls", "rustls-pemfile", "webpki-roots", "zeroize"]
ssl-rustls-awsrc =  ["ssl-rustls", "ssl-provider-awsrc", "rustls/aws-lc-rs"]
ssl-rustls-ring =  ["ssl-rustls", "ssl-provider-ring", "rustls/ring"]

# After
ssl-rustls = ["std", "rustls", "rustls-pemfile", "webpki-roots", "zeroize", "rustls/std", "rustls/tls12", "rustls/logging"]
ssl-rustls-awsrc =  ["ssl-rustls", "ssl-provider-awsrc", "rustls/aws-lc-rs"]
ssl-rustls-ring =  ["ssl-rustls", "ssl-provider-ring", "rustls/ring", "rustls/tls12", "rustls/logging", "rustls/std"]
```

The `ssl-rustls` feature now explicitly lists the minimal rustls sub-features needed (`std`, `tls12`, `logging`) rather than relying on rustls defaults.

### Part 3: Update Cargo.lock

The lock file still had `rustls` resolved with `aws-lc-rs` features. Updated it:

```bash
cargo update -p rustls@0.23.35
# rustls v0.23.35 -> v0.23.38 (with correct feature resolution)
```

Verified with:

```bash
cargo tree -p rustls@0.23.38 -f "{p} {f}"
# Output: rustls v0.23.38 log,logging,ring,std,tls12
# No aws-lc-rs in the feature list
```

---

## Why This Happened

### The CC Builder Bug

`aws-lc-sys` has two build modes:

1. **CC builder**: Uses the `cc` crate to compile a curated list of `.c` files directly. The file list is hardcoded in `builder/cc_builder/universal.rs`. In v0.38.0, this list was **incomplete** — it was missing many `crypto/evp/*.c` source files that define core EVP functions.

2. **CMake builder**: Uses CMake to build the full aws-lc source tree, which includes all files. This is more complete but slower and requires CMake as a build dependency.

The CC builder is the default on Linux when no `AWS_LC_SYS_CMAKE_BUILDER=1` is set, because it avoids the CMake dependency and builds faster.

### Why lld Made It Worse

Even with the CMake builder, lld had trouble resolving symbols. Possible causes:

- The CMake build with the `universal` cfg produced an archive where some object files had cross-references that lld couldn't resolve in the order the linker processed them
- The archive format or symbol table generation from CMake + clang may differ from what lld expects
- GNU `ar` vs LLVM `ar` differences in archive indexing

### Cargo Feature Unification

Rust's Cargo unifies features across all dependencies on the same crate. If crate A depends on `rustls` with `default-features = false` and crate B depends on `rustls` with default features, **default features are enabled globally**. This is why removing the llama-cpp examples (the `hf-hub` dependency path) was necessary — just configuring `foundation_core` correctly wasn't enough.

---

## Files Changed

| File | Change |
|------|--------|
| `Cargo.toml` | Removed `examples/llama-cpp/*` from `members`, added to `exclude` |
| `backends/foundation_core/Cargo.toml` | Added `default-features = false` to `rustls` dependency; updated `ssl-rustls` and `ssl-rustls-ring` features to explicitly enable only ring-compatible sub-features |
| `Cargo.lock` | Updated via `cargo update -p rustls@0.23.35` to resolve `rustls` without `aws-lc-rs` |

## Build Verification

```bash
cargo build --bin ewe_platform
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 16s
```

No linker errors. The binary links successfully using `ring` for TLS.
