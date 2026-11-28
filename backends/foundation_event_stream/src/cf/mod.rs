//! The Cloudflare Worker host surface.
//!
//! A Worker builds the shared [`SubscriptionEngine`](crate::shared::SubscriptionEngine) with the
//! durable object storage that implements
//! [`foundation_core::durable::DurableStreamVfs`](crate::durable::DurableStreamVfs), then serves
//! the control routes with its own request adapter. The core in [`crate::shared`] is the whole
//! surface; this module only names it for the Worker build.

pub use crate::shared::{
    SubscriptionConfig, SubscriptionEngine, SubscriptionError, SubscriptionErrorCode,
    SubscriptionRecord, WebhookSender, WebhookSigningKey,
};
