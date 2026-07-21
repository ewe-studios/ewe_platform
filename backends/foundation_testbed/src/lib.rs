//! foundation_testbed — a holistic test harness. Feature-gated capabilities:
//! `wasm` (a CLI-driven `wasm32-unknown-unknown` harness: browser via the
//! pure-Rust CDP/BiDi driver, Deno, and Cloudflare Workers).
//!
//! VM/container orchestration (QEMU, UTM, Docker) moved to
//! `foundation_deployment_platform` (spec-53, Feature 11). Re-exported here
//! for convenience.

#![allow(clippy::too_many_arguments)]

// Re-export the platform for backward compatibility. Only present when the
// (optional) platform dependency is pulled in — the `vms` feature (or its
// `foundation_deployment_platform_vms` alias). Without this gate the re-export
// fails to compile under the default feature set, which breaks every crate that
// dev-depends on `foundation_testbed`.
#[cfg(any(feature = "vms", feature = "foundation_deployment_platform_vms"))]
pub use foundation_deployment_platform as platform;

#[cfg(feature = "wasm")]
pub mod wasm;

// F52: re-export js-sys / web-sys / wasm-bindgen-futures for browser tests.
// wasm-bindgen-test must be a direct dev-dep of the consumer (Rust limitation:
// proc-macro attributes cannot be re-exported across crate boundaries).
#[cfg(feature = "wasm-bindgen-test")]
pub mod bindgen;

// F30: Docker-based cross-platform test environments.
#[cfg(feature = "docker-tests")]
pub mod docker;

// F32: QEMU Monitor Protocol client — universal agentic control (Tier 1).
// Always available (not feature-gated) — uses serde_json + Unix sockets.
pub mod qmp;
