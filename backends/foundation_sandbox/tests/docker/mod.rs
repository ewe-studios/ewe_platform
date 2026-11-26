//! The Docker driver tests (feature 10).
//!
//! `sim` uses the fake Engine and never starts a container. `e2e` uses the
//! Docker Engine of this host and returns with the probe report when the host
//! cannot run it.

mod e2e;
mod sim;
/// The fake Engine is shared with the image-import tests of feature 13.
pub mod support;
