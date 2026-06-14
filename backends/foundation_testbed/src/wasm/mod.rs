//! wasm testbed — a CLI-driven test harness for `wasm32-unknown-unknown`.
//!
//! Supports browser (pure-Rust CDP/BiDi driver), Deno, and Cloudflare Workers
//! (wrangler) execution, with both the custom harness and auto-generated
//! wasm-bindgen test modes. Gated behind the crate's `wasm` feature.

pub mod browser;
pub mod build;
pub mod cli;
pub mod deno;
pub mod error;
pub mod fwt;
pub mod fwt_runner;
pub mod init;
pub mod server;
pub mod wasm;
pub mod wasm_test;
pub mod wrangler;
