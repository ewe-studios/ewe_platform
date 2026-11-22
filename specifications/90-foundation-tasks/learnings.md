# Specification 90 learnings

## 2026-11-22

Cargo builds the current command as `ewe_platform`. Its help text uses `platform`.
The release workflow packages each executable target that Cargo reports.
The Makefile also names the current executable directly.
These callers require the compatibility entry. Both executable names can share one source.
[Manifest](../../bin/platform/Cargo.toml) · [Release workflow](../../.github/workflows/release.yaml) · [Makefile](../../Makefile)

The WebAssembly build and bundle commands already delegate to package libraries.
Each library can expose its existing operations through a Rust task set.
[Build adapter](../../bin/platform/src/wasm_bins/mod.rs) · [Bundle adapter](../../bin/platform/src/wasm_bundle/mod.rs)

The deployment package also supplies a standalone command.
Its separate entry demonstrates why a package needs control over runtime setup.
[Deployment entry](../../backends/foundation_deployment_platform/src/bin/testbed.rs)
