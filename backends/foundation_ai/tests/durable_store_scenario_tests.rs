//! Runnable target for the `DurableDocumentStore` document scenario suite.
//!
//! WHY: the suite lives in `tests/agentic/durable_store_scenario_tests.rs`. The
//! `agentic_tests` target requires the `testing` feature, and the default test
//! run does not enable that feature. This target keeps the suite runnable with
//! the default features, so `cargo test -p foundation_ai durable_store` runs it
//! and does not report a false pass over zero tests.
//!
//! The suite makes no network call and needs no feature.

#[path = "agentic/durable_store_scenario_tests.rs"]
mod durable_store_scenario_tests;
