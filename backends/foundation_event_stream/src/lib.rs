//! Durable Streams subscriptions for the platform.
//!
//! The crate owns the reserved `__ds` subscription surface of the Durable Streams protocol. It
//! re-exports [`foundation_core::durable`] as [`durable`], so a host imports the stream types and
//! the subscription API from one crate. It re-exports the JSON Web Key (JWK) types it signs with
//! from `foundation_auth`, so the platform keeps one key model.
//!
//! The split follows the rest of the platform:
//!
//! - [`shared`] holds the transport-neutral subscription core. It reads the injected clock,
//!   entropy and virtual file system (VFS) from [`foundation_context::Ctx`].
//! - [`native`] holds the Hypertext Transfer Protocol (HTTP) handlers and the native webhook
//!   sender. It is available on Unix targets only.
//! - [`cf`] exposes the shared core to a Cloudflare Worker host.
//!
//! A host supplies the webhook signing key and the webhook transport. The crate reads no global.

pub mod cf;
pub mod shared;

#[cfg(unix)]
pub mod native;

pub use foundation_auth::shared::jwks::{Jwk, Jwks, JwksError, JwksManager};

/// The Durable Streams protocol core.
///
/// This is a re-export of the whole module, not a copy of its types. A host imports stream types
/// and the subscription API from this crate.
pub use foundation_core::durable;

pub use shared::{
    DeliveryNotifier, LinkType, StreamInfo, StreamView, SubscriptionConfig, SubscriptionCreate,
    SubscriptionEngine, SubscriptionError, SubscriptionErrorCode, SubscriptionRecord,
    SubscriptionStatus, SubscriptionType, SubscriptionView, WebhookSignature, WebhookSigningKey,
};
