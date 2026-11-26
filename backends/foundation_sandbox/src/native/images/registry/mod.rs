//! The registry image source (feature 15).
//!
//! The module reads a container image from a registry with the Open Container
//! Initiative (OCI) distribution protocol. It has three parts:
//!
//! - [`reference`] parses `registry/repository:tag` and
//!   `registry/repository@sha256:<hex>` text.
//! - [`manifest`] holds the manifest and index documents and the platform
//!   choice.
//! - [`client`] reads the manifest and streams the blobs to the blob cache.
//!
//! Every body is a stream. A layer is never held whole in memory (invariant
//! I3).

#![allow(
    clippy::module_name_repetitions,
    reason = "the feature names the module registry and keeps the spec type names"
)]

pub mod auth;
pub mod client;
pub mod manifest;
pub mod reference;

pub use auth::{parse_www_authenticate, Challenge};
pub use client::{
    digest_hex, RegistryAuth, RegistryClient, RegistryLimits, RegistryOptions, ResolvedManifest,
};
pub use manifest::{
    check_image_arch, choose_platform, layer_kind, DocumentKind, LayerKind, OciDescriptor,
    OciImageConfigDocument, OciIndexEntry, OciManifestDocument, OciPlatform, GZIP_LAYER_TYPES,
    MANIFEST_ACCEPT, PLAIN_LAYER_TYPES,
};
pub use reference::{OciReference, TagOrDigest, DOCKER_HUB_REGISTRY};
