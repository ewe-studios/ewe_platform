//! Standard `` `OpenAPI` `` fetch utilities shared across providers.
//!
//! This module provides a generic fetch function for providers that use
//! standard `` `OpenAPI` `` 3.x specs without custom authentication requirements.
//!
//! Spec **normalization** used to live here (`normalize.rs`). It moved to
//! [`foundation_openapi::transform`] (spec-56 decision 05): canonicalising a spec
//! is a build-time concern owned by the spec-processing library and driven by
//! `genapi`, not something a runtime crate should carry.

pub mod fetch;
