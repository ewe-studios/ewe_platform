# Feature: build-verification

## Description

Verify that the llama.cpp build system works correctly after the submodule relocation and include path changes on all supported target platforms.

## Platforms to Verify

### Linux (x86_64-unknown-linux-gnu)

- [ ] `cargo check -p infrastructure_llama_bindings`
- [ ] `cargo check -p infrastructure_llama_bindings --features mtmd`
- [ ] `cargo build -p infrastructure_llama_bindings`
- [ ] `cargo build -p infrastructure_llama_bindings --features mtmd`
- [ ] `cargo build -p infrastructure_llama_bindings --features cuda` (if CUDA available)
- [ ] `cargo build -p infrastructure_llama_bindings --features vulkan` (if Vulkan available)

### macOS (aarch64-apple-darwin / x86_64-apple-darwin)

- [ ] `cargo check -p infrastructure_llama_bindings`
- [ ] `cargo build -p infrastructure_llama_bindings`
- [ ] `cargo build -p infrastructure_llama_bindings --features metal`
- [ ] `cargo build -p infrastructure_llama_bindings --features mtmd`

### Android (aarch64-linux-android)

- [ ] `cargo check -p infrastructure_llama_bindings --target aarch64-linux-android`
- [ ] Verify Android NDK is found (one of: `ANDROID_NDK`, `ANDROID_NDK_ROOT`, `NDK_ROOT`)
- [ ] Verify bindgen uses the correct Android sysroot clang

### WASM (wasm32-unknown-unknown) - if applicable

- [ ] `cargo check -p infrastructure_llama_bindings --target wasm32-unknown-unknown` (may be excluded or feature-gated)

## Additional Verification

- [ ] `mise run check` passes
- [ ] `mise run llama:server:build` succeeds (builds llama-server binary at `support/bin/llama-server`)
- [ ] `mise run llama:server:version` shows version info
- [ ] Downstream crate `foundation_ai` builds: `cargo build -p foundation_ai`
- [ ] `foundation_ai` tests pass: `cargo test -p foundation_ai --lib`
- [ ] No stale references to old path: `grep -r "infrastructure/llama-bindings/llama.cpp" --include="*.toml" --include="*.rs" --include="*.h" .`

## bindgen Output Verification

After a successful build, verify that the generated bindings file contains:
- `llama_init_from_file` or similar core llama functions
- `ggml_*` type and function declarations
- If `mtmd` feature enabled: `mtmd_*` function and type declarations

## Regression Check

- [ ] Verify that the generated bindings are functionally equivalent to before the move (same symbols, same types)
- [ ] Run any existing tests in `infrastructure/llama-bindings/` if present
