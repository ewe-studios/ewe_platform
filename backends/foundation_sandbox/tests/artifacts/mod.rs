//! The guest-artifact tests (feature 03).
//!
//! `cpio_kernel_accepts_it` (a kernel boots the archive) belongs to the microVM
//! tests of feature 05. It is named in the feature file so the link is clear.
#![allow(
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    reason = "the tests read clearer with these forms"
)]

mod support;

mod cpio;
mod fetch;
mod net;
mod store;
mod tarball;
