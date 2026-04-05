//! Foundation DB - Unified Storage Backend
//!
//! Provides a consistent abstraction layer for persisting data across multiple storage providers:
//! - Turso - SQLite-compatible embedded/remote database with sync API
//! - Cloudflare D1 - Edge `SQLite` for Cloudflare Workers (TODO)
//! - Cloudflare R2 - Object storage for larger blobs (TODO)
//! - In-Memory - Ephemeral storage for development/testing

mod backends;
mod crypto;
mod errors;
mod rows_stream;
mod schema;
pub mod state;
mod storage_provider;

pub use backends::*;
pub use crypto::*;
pub use errors::*;
pub use rows_stream::*;
pub use schema::*;
pub use state::*;
pub use storage_provider::*;
