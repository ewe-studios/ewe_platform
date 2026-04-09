//! GCP API providers with automatic state tracking.
//!
//! WHY: Users need stateful API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Per-API provider implementations using StoreStateIdentifierTask.
//!
//! HOW: Each provider wraps ProviderClient<S> and provides methods
//!      that automatically store state on successful operations.

#![cfg(feature = "gcp")]

pub mod cloudkms;
