//! High-level HTTP client with clean public API — native (wasm32-gated).
//!
//! `SimpleHttpClient` is **deprecated** — use [`NativeHttpClient`] instead.
//! F51 (unified network client) folded `SimpleHttpClient` into
//! `NativeHttpClient`, which now owns the pool, config, TLS connector, verb
//! builders, timeouts, proxy, redirect, and retry logic directly.
//!
//! This file exists as a compatibility re-export so the ~23K generated and
//! hand-written references keep compiling through the migration window. It will
//! be removed in Stage 4 when all sites have been migrated.

use crate::simple_http::client::shared::{DnsResolver, SystemDnsResolver};
use crate::simple_http::client::NativeHttpClient;

#[deprecated(note = "use NativeHttpClient; SimpleHttpClient was folded into it (F51)")]
#[allow(type_alias_bounds)]
pub type SimpleHttpClient<R: DnsResolver = SystemDnsResolver> = NativeHttpClient<R>;
