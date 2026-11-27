//! The `Sandboxfile` builder tests (feature 14, section 7).
//!
//! The parser and key rows need no sandbox. The executor rows use the
//! simulated driver and the scripted slave of the control-plane tests, with a
//! real temporary cache directory: no virtual machine and no Docker run.
//!
//! The end-to-end rows are in `tests/e2e_images/mod.rs`; they need a Linux host
//! with a usable microVM driver.

mod add;
mod fetch;
mod keys;
mod parse;
mod sim;
mod support;
