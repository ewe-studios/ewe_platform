//! Spec 81 feature 00, task T11: the client-authentication providers.
//!
//! `cargo test -p foundation_auth --test client_auth_tests` runs all of them.

mod any_of;
mod common;
mod forwarded;
mod insecure;
mod jwt;
mod signed_secret;
