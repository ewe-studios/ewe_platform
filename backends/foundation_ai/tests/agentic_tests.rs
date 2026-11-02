//! The `agentic_tests` integration-test crate root.
//!
//! The tests live in `tests/agentic/`. That directory is also the `agentic`
//! module of the `foundation_ai_tests` crate, loaded through `tests/mod.rs`.
//! This thin root loads the same module tree and holds the crate-level
//! `recursion_limit`. The deep `Send` auto-trait resolution in
//! `session_mock_tests.rs` needs the higher limit. The attribute stays here:
//! an inner attribute in `tests/agentic/mod.rs` would warn `unused_attributes`
//! in the other crate.

#![recursion_limit = "256"]

#[path = "agentic/mod.rs"]
mod agentic;
