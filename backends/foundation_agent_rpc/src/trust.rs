//! The trusted-client store, re-exported from `foundation_auth` (spec 87).
//!
//! WHY: spec 87 moved the certificate authority, the trust store and the
//! verifier into `foundation_auth`, so every host shares one implementation.
//! This module keeps the agent RPC's public paths stable and names the file
//! that the host keeps in its state directory.
//!
//! WHAT: the trust-store types of `foundation_auth::trust`, plus
//! [`TRUSTED_CLIENTS_DB_FILE`].
//!
//! HOW: a re-export, not a copy. The agent RPC has no trust store of its own.

/// File name of the default SQLite database inside a state directory.
pub const TRUSTED_CLIENTS_DB_FILE: &str = "trusted-clients.db";

pub use foundation_auth::trust::{
    IndexMode, KvTrustStore, MemoryTrustStore, SecretUse, SqlTrustStore, TrustConfig, TrustError,
    TrustFuture, TrustIndex, TrustStore, TrustStoreInfo, TrustedClient, TrustedClientStream,
    TrustedClients, UseOutcome, DEFAULT_TABLE_PREFIX,
};
