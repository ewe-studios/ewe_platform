//! The sandbox image tests of feature 12.
//!
//! The store tests use real files in a temporary directory. The control tests
//! use the simulated driver and the scripted slave, so no virtual machine runs.

mod support;

mod control;
/// The libkrun disk tests need the libkrun driver and its simulation helpers.
#[cfg(all(unix, feature = "libkrun"))]
mod disks;
mod fault;
/// The container image source tests (feature 13). They need the fake Docker
/// Engine, so the `images` feature (which pulls `docker`) gates them.
#[cfg(feature = "images")]
mod import;
/// The registry authentication tests (feature 15, section 6.3).
#[cfg(feature = "images")]
mod oci_auth;
/// The registry reference tests (feature 15, section 6.1).
#[cfg(feature = "images")]
mod oci_reference;
/// The registry client tests (feature 15, section 6.2).
#[cfg(feature = "images")]
mod oci_registry;
/// The fake registry of the feature 15 tests.
#[cfg(feature = "images")]
mod registry_server;
mod store;
mod unpack;
mod user;
mod writable;
