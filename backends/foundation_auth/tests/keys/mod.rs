//! Spec 81 feature 00, task T7: operator key files.
//!
//! `cargo test -p foundation_auth --test keys_tests` runs all of them.

// The native suite reads the file system and the operating system.
#![cfg(not(target_arch = "wasm32"))]

mod common;
mod debug_output;
mod formats_agree;
mod legacy_state_dir;
mod load;
mod passphrase;
#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
mod rsa;
