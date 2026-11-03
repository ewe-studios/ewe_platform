//! Correctness follow-ups (spec 70 feature 74).
//!
//! Every test has a hard time limit (`timeout`, in milliseconds).

#![cfg(feature = "server-test")]

mod cache_guard;
mod cookies;
mod enrollment;
mod register;
mod social_code;
mod social_tos;
mod sweeper;
