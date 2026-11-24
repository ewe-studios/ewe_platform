//! Spec 81 feature 00, task T5: signed secrets.
//!
//! `cargo test -p foundation_auth --test secret_tests` runs all of them.
//! `cargo test -p foundation_auth --test secret_tests golden_v1` runs one
//! group.

mod common;
mod enroll_proof;
mod golden_v1;
mod pkcs8;
#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
mod rsa;
mod v2;
mod verify_order;
