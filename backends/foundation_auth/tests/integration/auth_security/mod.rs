//! Security follow-ups (spec 70 feature 73): single-use take, cache expiry,
//! TOTP enrollment, social redirect checks, logout and failed attempts.
//!
//! Every test has a hard time limit (`timeout`, in milliseconds).

#![cfg(feature = "server-test")]

mod expiry;
mod single_use;
mod social;
mod totp;
