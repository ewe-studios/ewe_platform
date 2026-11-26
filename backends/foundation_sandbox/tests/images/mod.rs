//! The sandbox image tests of feature 12.
//!
//! The store tests use real files in a temporary directory. The control tests
//! use the simulated driver and the scripted slave, so no virtual machine runs.

mod support;

mod control;
/// The libkrun disk tests need the libkrun driver and its simulation helpers.
#[cfg(all(unix, feature = "libkrun"))]
mod disks;
mod store;
mod unpack;
mod user;
mod writable;
