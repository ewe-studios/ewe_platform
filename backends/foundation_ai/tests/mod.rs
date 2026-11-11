//! `foundation_ai_tests` integration-test crate root.
//!
//! The `agentic` module compiles `tests/agentic/session_mock_tests.rs`. Its
//! `TursoStorage` test drives a deep `Send` auto-trait resolution. The higher
//! recursion limit keeps that resolution under the future hard-error depth.

#![recursion_limit = "256"]

mod agentic;
mod candle;
mod gpu;
mod harness;
mod memory;
mod models;
mod providers;
mod tools;
