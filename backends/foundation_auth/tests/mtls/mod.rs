//! Spec 87 tests: the certificate authority, the trust store, the mutual-TLS
//! (mTLS) verifier and the mTLS providers.
//!
//! The module names let the verify commands of the specification select one
//! group: `cargo test -p foundation_auth pki` runs `pki::*`.

mod client_auth;
mod mtls;
mod pki;
mod trust;
