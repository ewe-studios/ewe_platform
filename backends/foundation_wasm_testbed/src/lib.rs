//! CLI-driven test harness for wasm32-unknown-unknown.
//!
//! Supports browser (Playwright), Deno, and Cloudflare Workers (wrangler) execution,
//! with both custom harness and auto-generated wasm-bindgen test modes.

mod browser;
mod build;
mod cli;
mod deno;
mod init;
mod server;
mod wasm;
mod wasm_test;
mod wrangler;
