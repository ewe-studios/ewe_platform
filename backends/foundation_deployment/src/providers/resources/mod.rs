//! Provider OpenAPI spec fetchers.
//!
//! WHY: Fetches OpenAPI specifications from cloud providers for type generation.
//!
//! WHAT: Provider-specific fetchers that store raw JSON specs and generate
//! Rust resource types.
//!
//! HOW: Each provider module implements custom fetch logic (HTTP, git clone, etc.)
//! and stores specs in the provider's resource_specs directory.

pub mod cloudflare;
pub mod gcp;
pub mod standard;
