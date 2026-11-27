//! Tests for the S3 and R2 durable stream virtual file system (VFS).

mod conformance;
mod coverage;
#[cfg(feature = "vfs-object-http")]
mod blocks;
#[cfg(feature = "vfs-object-http")]
mod http;
#[cfg(feature = "vfs-object-netio")]
mod live;
#[cfg(feature = "vfs-object-netio")]
mod netio;
#[cfg(feature = "vfs-object-http")]
mod sql_support;
mod support;
