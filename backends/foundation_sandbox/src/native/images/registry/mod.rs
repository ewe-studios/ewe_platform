//! The registry image source (feature 15).
//!
//! The module reads a container image from a registry with the Open Container
//! Initiative (OCI) distribution protocol. It has three parts:
//!
//! - [`reference`] parses `registry/repository:tag` and
//!   `registry/repository@sha256:<hex>` text.
//! - the client reads the manifest and streams the blobs to the blob cache.
//! - the authentication part gets a token when the registry asks for one.
//!
//! Every body is a stream. A layer is never held whole in memory (invariant
//! I3).

#![allow(
    clippy::module_name_repetitions,
    reason = "the feature names the module registry and keeps the spec type names"
)]

pub mod reference;

pub use reference::{OciReference, TagOrDigest, DOCKER_HUB_REGISTRY};
