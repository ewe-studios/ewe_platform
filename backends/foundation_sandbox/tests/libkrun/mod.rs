//! The libkrun driver tests (feature 09).
//!
//! The driver tests need no virtual machine: they use a fake process spawner
//! and an in-memory host file system. The end-to-end tests in [`e2e`] need the
//! real library and a hypervisor, and they return with a report when the host
//! cannot run them.

mod driver;
mod e2e;
mod support;
