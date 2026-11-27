//! Common shared types for all providers.
//!
//! The types moved to the small `foundation_api_common` crate (spec 77 feature
//! 11), so a client that needs them does not depend on the database and cloud
//! providers of this crate. This module re-exports them, and every current
//! user compiles with no change.

pub mod multipart;

pub use foundation_api_common::*;
pub use multipart::{MultipartBody, MultipartPart, DEFAULT_BOUNDARY};
