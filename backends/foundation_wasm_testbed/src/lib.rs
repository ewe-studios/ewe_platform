//! CLI-driven test harness for wasm32-unknown-unknown.
//!
//! Supports browser (Playwright), Deno, and Cloudflare Workers (wrangler) execution,
//! with both custom harness and auto-generated wasm-bindgen test modes.

pub mod browser;
pub mod build;
pub mod fwt;
pub mod cli;
pub mod deno;
pub mod error;
pub mod init;
pub mod server;
pub mod wasm;
pub mod wasm_test;
pub mod wrangler;
