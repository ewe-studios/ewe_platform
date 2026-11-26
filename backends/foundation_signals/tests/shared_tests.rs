//! Integration tests for the native shared signal runtime.

#![cfg(all(feature = "thread_safe", not(target_family = "wasm")))]

#[path = "shared_tests/mod.rs"]
mod shared_tests;
