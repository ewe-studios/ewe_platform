//! Foundation DB - Unified Storage Backend
//!
//! Provides a consistent abstraction layer for persisting data across multiple storage providers:
//! - Turso (libsql) - Local/remote SQLite with edge sync
//! - Cloudflare D1 - Edge SQLite for Cloudflare Workers
//! - Cloudflare R2 - Object storage for larger blobs
//! - In-Memory - Ephemeral storage for development/testing

mod backends;
mod crypto;
mod errors;
mod schema;
mod storage_provider;

pub use backends::*;
pub use crypto::*;
pub use errors::*;
pub use schema::*;
pub use storage_provider::*;
