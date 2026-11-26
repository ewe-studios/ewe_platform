//! The trust store: which clients are enrolled, which may call, and how often
//! a bootstrap secret was used.
//!
//! WHY: a certificate that chains to the certificate authority (CA) proves
//! that a client enrolled once. It does not prove that the holder may call.
//! The trust store records each enrolled client, its role and its limits, and
//! a revocation stops the next call.
//!
//! WHAT: the [`TrustStore`] trait and three backends ([`MemoryTrustStore`],
//! [`SqlTrustStore`], [`KvTrustStore`]), the [`TrustedClient`] record, the
//! [`TrustConfig`] limits, and [`TrustedClients`], which adds an in-memory
//! index for the synchronous Transport Layer Security (TLS) verifier.
//!
//! HOW: writes go to the store first, then to the index. A read never depends
//! on a write that failed.

mod client;
mod clients;
mod config;
mod kv;
mod memory;
mod sql;

pub use client::{
    ClientKeyRecord, SecretUse, TrustError, TrustFuture, TrustStore, TrustStoreInfo, TrustedClient,
    TrustedClientStream, UseOutcome,
};
pub use clients::{TrustIndex, TrustedClients};
pub use config::{IndexMode, TrustConfig};
pub use kv::KvTrustStore;
pub use memory::MemoryTrustStore;
pub use sql::{SqlTrustStore, DEFAULT_TABLE_PREFIX};
