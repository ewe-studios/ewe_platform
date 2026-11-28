//! Wasm-only backends.
//!
//! A Cloudflare Worker has no local file system. Its durable edge path is the
//! Structured Query Language (SQL) virtual file system (VFS) on Durable Object
//! SQLite, with Cloudflare R2 as the immutable block tier.

#[cfg(feature = "vfs-object-wasm")]
pub mod vfs;
