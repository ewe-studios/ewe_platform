//! The sandbox image tests of feature 12.
//!
//! The store tests use real files in a temporary directory. The control tests
//! use the simulated driver and the scripted slave, so no virtual machine runs.

mod support;

mod control;
mod store;
