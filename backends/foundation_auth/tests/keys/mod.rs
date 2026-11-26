//! Spec 81 feature 00, task T7: operator key files.
//!
//! `cargo test -p foundation_auth --test keys_tests` runs all of them.

mod common;
mod debug_output;
mod formats_agree;
mod legacy_state_dir;
mod load;
mod passphrase;
#[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
mod rsa;
