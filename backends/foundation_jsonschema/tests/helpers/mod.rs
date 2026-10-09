//! Shared test utilities for foundation_jsonschema test suites.
//!
//! WHY: Multiple test integration files need the same helpers:
//! schema compilation, assertion helpers, and remote schema loading.
//! Centralizing them avoids duplication and keeps tests readable.
//!
//! WHAT: Common functions for running official test suite JSON files,
//! loading remote schemas, and creating validators for specific drafts.
//!
//! HOW: Import from `helpers` module in integration tests.

pub mod test_retriever;
