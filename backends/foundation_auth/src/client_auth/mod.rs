//! The client-authentication providers of spec 87.
//!
//! [`mtls::MtlsAuth`] reads the client certificate of a native connection.
//! The Cloudflare providers read the certificate facts that Cloudflare checked
//! at its edge.

pub mod cloudflare;
#[cfg(not(target_family = "wasm"))]
pub mod mtls;

pub use cloudflare::{CfMtlsAuth, CfTlsClientAuth};
#[cfg(not(target_family = "wasm"))]
pub use mtls::{MtlsAuth, SeenErrorPolicy};
