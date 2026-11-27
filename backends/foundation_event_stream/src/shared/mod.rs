//! The transport-neutral subscription core.
//!
//! Nothing here names Hypertext Transfer Protocol (HTTP), a socket or a global runtime. The host
//! supplies the clock and entropy through [`foundation_context::Ctx`], storage through a
//! [`foundation_core::durable::DurableStreamVfs`], and the webhook transport through
//! [`webhook::WebhookSender`].

pub mod config;
pub mod engine;
pub mod error;
pub mod glob;
pub mod hash;
pub mod model;
pub mod offsets;
pub mod signing;
pub mod store;
pub mod url;
pub mod webhook;

pub use config::{normalize_relative_path, SubscriptionConfig};
pub use engine::{DeliveryNotifier, SubscriptionEngine};
pub use error::{SubscriptionError, SubscriptionErrorCode};
pub use glob::glob_match;
pub use hash::stable_config_hash;
pub use model::{
    AckEntry, AckRequest, AckResponse, ClaimRequest, ClaimResponse, CreateRequest, LinkType,
    StreamInfo, StreamLink, StreamView, StreamsRequest, SubscriptionCreate, SubscriptionRecord,
    SubscriptionStatus, SubscriptionType, SubscriptionView, WakeEvent, WakeNotification,
    WebhookCreate, WebhookInput, WebhookSigningView, WebhookView,
};
pub use offsets::{compare_offsets, offset_from_len, zero_offset, BEFORE_FIRST};
pub use signing::{WebhookSignature, WebhookSigningKey};
pub use store::{ObservingVfs, StreamWriteObserver, SubscriptionStore};
pub use url::validate_webhook_url;
pub use webhook::{WebhookFuture, WebhookRequest, WebhookResponse, WebhookSender};
