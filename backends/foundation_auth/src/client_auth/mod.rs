//! The client-authentication providers.
//!
//! This module holds the spec 81 providers ([`SignedSecretAuth`],
//! [`JwtAuth`], [`InsecureNoAuth`], [`ForwardedPrincipalAuth`], [`AnyOf`]) and
//! names the spec 87 mutual-TLS (mTLS) providers ([`MtlsAuth`],
//! [`CfTlsClientAuth`], [`CfMtlsAuth`]) without defining them.

pub mod any_of;
pub mod forwarded;
pub mod insecure;

#[cfg(feature = "jwt")]
pub mod jwt;
#[cfg(feature = "secret")]
pub mod signed_secret;

pub mod cloudflare;
#[cfg(not(target_family = "wasm"))]
pub mod mtls;

pub use any_of::AnyOf;
pub use forwarded::{ForwardedPrincipalAuth, FORWARDED_PRINCIPAL_HEADER};
pub use insecure::InsecureNoAuth;

#[cfg(feature = "jwt")]
pub use jwt::JwtAuth;
#[cfg(feature = "secret")]
pub use signed_secret::{AccessPolicy, SignedSecretAuth};

pub use cloudflare::{CfMtlsAuth, CfTlsClientAuth};
#[cfg(not(target_family = "wasm"))]
pub use mtls::{MtlsAuth, SeenErrorPolicy};

// The auth primitives of spec 86, at the path that a provider user expects.
pub use crate::server_credential::{
    AuthError, AuthFuture, AuthInfo, AuthMethod, CallCredentials, ClientAuth, Principal,
};
