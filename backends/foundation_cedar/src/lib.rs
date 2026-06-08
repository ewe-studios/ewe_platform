//! Cedar policy engine wrapper for ewe_platform.
//!
//! Wraps the `cedar-policy` crate with pluggable policy storage,
//! entity providers, and an ergonomic builder API.

pub mod core;
pub mod storage;

pub use self::core::engine::CedarEngine;
pub use self::core::errors::CedarError;
pub use self::core::policy::{
    EntityProvider, JsonEntityProvider, StaticEntityProvider, parse_policies,
};
pub use self::core::request::{CedarRequest, CedarRequestBuilder};
pub use self::core::response::CedarResponse;
pub use self::storage::{InMemoryPolicyStore, PolicyStore, load_from_store};
