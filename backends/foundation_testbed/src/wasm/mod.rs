//! wasm testbed — a CLI-driven test harness for the wasm target matrix.
//!
//! Supports browser (pure-Rust CDP/BiDi driver), Deno, and Cloudflare Workers
//! (wrangler) execution, with both the custom harness and auto-generated
//! wasm-bindgen test modes. The `--target` flag selects the compilation target
//! (unknown-unknown, emscripten, wasip1, wasip2). Gated behind the crate's
//! `wasm` feature.

pub mod browser;
pub mod build;
pub mod cli;
pub mod deno;
// spec-44: in-process JS via the embedded Deno runtime (extra sub-feature).
#[cfg(feature = "wasm-embedded-js")]
pub mod embedded_js;
pub mod emscripten_runner;
pub mod error;
pub mod fwt;
pub mod fwt_runner;
pub mod init;
pub mod server;
pub mod wasm;
pub mod wasi_runner;
pub mod wasm_test;
pub mod wrangler;
