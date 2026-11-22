//! Transport Layer Security (TLS) verification and configuration.
//!
//! WHY: a mutual-TLS (mTLS) server needs one verifier and one set of
//! configuration builders. Without them every host writes its own, and a
//! checker cannot compare two hosts.
//!
//! WHAT: [`ClientCertVerifier`] (a pure chain check plus a trusted-client
//! index), [`CtxTimeProvider`], [`ServerTlsConfig`], [`MtlsConfig`], the
//! configuration builders and [`peer_identity`].
//!
//! HOW: the verifier is synchronous, because `rustls` calls it in the middle of
//! the handshake. It does no Input/Output (I/O) and starts no task. The
//! last-seen write belongs to the mTLS provider, on the call path.
//!
//! Native only: it uses `rustls`.

mod config;
mod time;
mod verifier;

pub use config::{
    client_config_enrollment, client_config_mutual, client_config_pinned,
    client_config_pinned_default, crypto_provider, server_config_mutual, server_config_server_auth,
    MtlsConfig, ServerTlsConfig,
};
pub use time::CtxTimeProvider;
pub use verifier::{
    peer_identity, tls_info_of, ClientCertVerifier, PeerCertificate, TrustedClientVerifier,
};
