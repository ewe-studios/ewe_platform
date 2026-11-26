//! The common API types of the generated provider clients.
//!
//! WHY: Every generated provider client needs the same request sender, the
//! same error type and the same streamed response bodies. A small crate holds
//! them, so a client that needs them does not pull the database and cloud
//! providers of `foundation_deployment` into its build. The static guest slave
//! of spec 77 needs this: a database crate has C code, and a static musl
//! program has no C library.
//!
//! WHAT: `ApiClient`, `ApiError`, `ApiResponse`, `ApiPending`, `BodyLimits`,
//! `Empty`, `Operation`, the senders and the streamed bodies.
//!
//! HOW: `foundation_deployment::providers::common` re-exports this crate, so a
//! current user compiles with no change. The generator of
//! `foundation_openapi` can name this crate as the import path.
//!
//! This crate is written in ASD-STE100 Simplified Technical English (STE).
//! Code and identifiers stay as they are.

pub mod api_types;
pub mod body;

pub use api_types::*;
pub use body::{
    read_bounded, read_error_body, with_raw_body, BodyLimits, ByteStream, JsonLinesStream,
};
