//! Portable domain logic for the keychain.
//!
//! No platform deps, no HTTP, no I/O. Just types, validation, and
//! trait definitions that the server backends implement.

pub mod api;
pub mod auth;
pub mod context;
pub mod crypto;
pub mod error;
pub mod models;
pub mod notifications;
pub mod store;
pub mod util;
