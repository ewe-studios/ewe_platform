//! foundation_testbed — a holistic test harness. Feature-gated capabilities:
//! `vms` (QEMU/KVM cross-platform VM build & test) and `wasm` (a CLI-driven
//! `wasm32-unknown-unknown` harness: browser via the pure-Rust CDP/BiDi driver,
//! Deno, and Cloudflare Workers).

#![allow(clippy::too_many_arguments)]

#[cfg(feature = "vms")]
pub mod vms;

#[cfg(feature = "wasm")]
pub mod wasm;

// F52: re-export js-sys / web-sys / wasm-bindgen-futures for browser tests.
// wasm-bindgen-test must be a direct dev-dep of the consumer (Rust limitation:
// proc-macro attributes cannot be re-exported across crate boundaries).
#[cfg(feature = "wasm-bindgen-test")]
pub mod bindgen;
