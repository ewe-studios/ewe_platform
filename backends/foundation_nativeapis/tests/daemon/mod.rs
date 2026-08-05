//! Integration tests for the DaemonMaster supervisor (spec-58 F01 + F02).
//!
//! Split by concern: pure config/dependency-graph tests (no pool), readiness
//! strategy tests (drive the valtron task), process lifecycle tests (spawn real
//! short-lived children on unix), and macro expansion tests.

mod config_tests;
mod deps_tests;
mod readiness_tests;

#[cfg(unix)]
mod lifecycle_tests;

#[cfg(unix)]
mod macro_tests;
