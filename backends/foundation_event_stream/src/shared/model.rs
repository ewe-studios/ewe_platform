//! Subscription data types, request bodies and response views.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The delivery mechanism of a subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubscriptionType {
    /// The server calls a webhook uniform resource locator (URL).
    Webhook,
    /// The server writes wake events to a durable stream.
    PullWake,
}

impl SubscriptionType {
    /// The wire text of the type.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webhook => "webhook",
            Self::PullWake => "pull-wake",
        }
    }
}

/// The delivery status of a subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    /// Delivery works or is idle.
    Active,
    /// A webhook attempt failed and a retry is scheduled.
    Failed,
}

impl SubscriptionStatus {
    /// The wire text of the status.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Failed => "failed",
        }
    }
}

/// How a stream joined a subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    /// The stream matches the subscription pattern.
    Glob,
    /// An explicit `streams[]` entry named the stream.
    Explicit,
}

impl LinkType {
    /// The wire text of the link type.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Glob => "glob",
            Self::Explicit => "explicit",
        }
    }
}

/// One linked stream and its acknowledged cursor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamLink {
    /// The stream-root-relative path.
    pub path: String,
    /// Every way the stream joined the subscription.
    pub link_types: BTreeSet<LinkType>,
    /// The last processed offset, inclusive, as wire text. `-1` means before the first offset.
    pub acked_offset: String,
}

impl StreamLink {
    /// The serialized link type. Explicit wins when a stream has both links.
    #[must_use]
    pub fn link_type(&self) -> LinkType {
        if self.link_types.contains(&LinkType::Explicit) {
            LinkType::Explicit
        } else {
            LinkType::Glob
        }
    }
}

/// One stored subscription.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionRecord {
    /// The client-provided identifier.
    pub id: String,
    /// The delivery mechanism.
    #[serde(rename = "type")]
    pub kind: SubscriptionType,
    /// The glob over stream-root-relative paths.
    pub pattern: Option<String>,
    /// The webhook uniform resource locator (URL) for a webhook subscription.
    pub webhook_url: Option<String>,
    /// The wake stream path for a pull-wake subscription.
    pub wake_stream: Option<String>,
    /// The lease time to live (TTL) in milliseconds.
    pub lease_ttl_ms: u64,
    /// The human-readable description.
    pub description: Option<String>,
    /// The creation time in RFC 3339 form.
    pub created_at: String,
    /// The delivery status.
    pub status: SubscriptionStatus,
    /// The hash of the normalized configuration.
    pub config_hash: String,
    /// The public base uniform resource locator (URL) used for callback and JWKS links.
    pub callback_base_url: String,
    /// The linked streams by path.
    pub streams: BTreeMap<String, StreamLink>,
    /// The generation counter. It increments for every wake.
    pub generation: u64,
    /// The current wake identifier.
    pub wake_id: Option<String>,
    /// The acknowledged offsets captured by the current wake, by stream path.
    pub wake_snapshot: BTreeMap<String, String>,
    /// The bearer token of the current or last wake.
    pub token: Option<String>,
    /// The generation the token was issued for.
    pub token_generation: u64,
    /// The worker that holds the lease, for a pull-wake subscription.
    pub holder: Option<String>,
    /// The wall-clock time, in UNIX milliseconds, when the lease ends.
    pub lease_expires_at_ms: Option<i64>,
    /// The failed delivery attempts since the last success.
    pub retry_count: u32,
    /// The wall-clock time, in UNIX milliseconds, of the next webhook retry.
    pub next_attempt_at_ms: Option<i64>,
}

impl SubscriptionRecord {
    /// True when a wake is in flight or a worker holds the lease.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.wake_id.is_none() && self.holder.is_none()
    }

    /// Clear the wake, the lease and the snapshot.
    pub fn clear_wake(&mut self) {
        self.holder = None;
        self.wake_id = None;
        self.wake_snapshot.clear();
        self.lease_expires_at_ms = None;
    }
}

/// The normalized subscription configuration.
///
/// The configuration hash covers every field, so a re-confirm with a change is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SubscriptionCreate {
    /// The delivery mechanism.
    #[serde(rename = "type")]
    pub kind: SubscriptionType,
    /// The glob over stream-root-relative paths.
    pub pattern: Option<String>,
    /// The explicit stream paths, sorted and without duplicates.
    pub streams: Vec<String>,
    /// The webhook configuration.
    pub webhook: Option<WebhookCreate>,
    /// The wake stream path.
    pub wake_stream: Option<String>,
    /// The lease time to live (TTL) in milliseconds.
    pub lease_ttl_ms: u64,
    /// The human-readable description.
    pub description: Option<String>,
}

/// The webhook part of a normalized configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WebhookCreate {
    /// The webhook uniform resource locator (URL).
    pub url: String,
}

/// One stream in a wake notification or claim response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamInfo {
    /// The stream-root-relative path.
    pub path: String,
    /// How the stream joined the subscription.
    pub link_type: LinkType,
    /// The last processed offset, inclusive.
    pub acked_offset: String,
    /// The current tail offset.
    pub tail_offset: String,
    /// True when data after `acked_offset` waits.
    pub has_pending: bool,
}

/// One stream in a subscription read response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StreamView {
    /// The stream-root-relative path.
    pub path: String,
    /// How the stream joined the subscription.
    pub link_type: LinkType,
    /// The last processed offset, inclusive.
    pub acked_offset: String,
}

/// The webhook signing metadata in a subscription read response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WebhookSigningView {
    /// The signature algorithm. The protocol fixes it to `ed25519`.
    pub alg: String,
    /// The active signing key identifier.
    pub kid: String,
    /// The JSON Web Key Set (JWKS) endpoint.
    pub jwks_url: String,
}

/// The webhook part of a subscription read response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WebhookView {
    /// The webhook uniform resource locator (URL).
    pub url: String,
    /// The signing metadata. The response never includes a secret.
    pub signing: WebhookSigningView,
}

/// The subscription read response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SubscriptionView {
    /// The identifier.
    pub id: String,
    /// The identifier again, under its protocol name.
    pub subscription_id: String,
    /// The delivery mechanism.
    #[serde(rename = "type")]
    pub kind: SubscriptionType,
    /// The glob over stream-root-relative paths.
    pub pattern: Option<String>,
    /// The linked streams.
    pub streams: Vec<StreamView>,
    /// The webhook configuration for a webhook subscription.
    pub webhook: Option<WebhookView>,
    /// The wake stream path for a pull-wake subscription.
    pub wake_stream: Option<String>,
    /// The lease time to live (TTL) in milliseconds.
    pub lease_ttl_ms: u64,
    /// The creation time in RFC 3339 form.
    pub created_at: String,
    /// The delivery status.
    pub status: SubscriptionStatus,
    /// The human-readable description.
    pub description: Option<String>,
}

/// The webhook body that wakes a handler.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakeNotification {
    /// The subscription identifier.
    pub subscription_id: String,
    /// The wake identifier.
    pub wake_id: String,
    /// The subscription generation.
    pub generation: u64,
    /// The linked streams and their offsets.
    pub streams: Vec<StreamInfo>,
    /// The callback uniform resource locator (URL).
    pub callback_url: String,
    /// The scoped callback token.
    pub callback_token: String,
}

/// The successful pull-wake claim response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ClaimResponse {
    /// The wake identifier.
    pub wake_id: String,
    /// The subscription generation.
    pub generation: u64,
    /// The scoped claim token.
    pub token: String,
    /// The linked streams and their offsets.
    pub streams: Vec<StreamInfo>,
    /// The lease time to live (TTL) in milliseconds.
    pub lease_ttl_ms: u64,
}

/// The successful ack and callback response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AckResponse {
    /// Always true on success.
    pub ok: bool,
    /// True when the subscription already has another wake.
    pub next_wake: bool,
}

/// The pull-wake event written to the wake stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakeEvent {
    /// The event kind. The protocol fixes it to `wake`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The subscription identifier.
    pub subscription_id: String,
    /// The stream that received the append.
    pub stream: String,
    /// The subscription generation.
    pub generation: u64,
    /// The event time in UNIX milliseconds.
    pub ts: i64,
}

/// The create or re-confirm request body.
#[derive(Clone, Debug, Deserialize)]
pub struct CreateRequest {
    /// The delivery mechanism.
    #[serde(rename = "type")]
    pub kind: String,
    /// The glob over stream-root-relative paths.
    pub pattern: Option<String>,
    /// The explicit stream paths.
    #[serde(default)]
    pub streams: Vec<String>,
    /// The webhook configuration.
    pub webhook: Option<WebhookInput>,
    /// The wake stream path.
    pub wake_stream: Option<String>,
    /// The lease time to live (TTL) in milliseconds.
    pub lease_ttl_ms: Option<u64>,
    /// The human-readable description.
    pub description: Option<String>,
}

/// The webhook part of a create request.
#[derive(Clone, Debug, Deserialize)]
pub struct WebhookInput {
    /// The webhook uniform resource locator (URL).
    pub url: String,
}

/// The explicit stream membership request body.
#[derive(Clone, Debug, Deserialize)]
pub struct StreamsRequest {
    /// The stream paths to add.
    pub streams: Vec<String>,
}

/// The claim request body.
#[derive(Clone, Debug, Deserialize)]
pub struct ClaimRequest {
    /// The worker name.
    pub worker: String,
}

/// One acknowledged stream offset.
#[derive(Clone, Debug, Deserialize)]
pub struct AckEntry {
    /// The stream path, under its protocol name.
    pub stream: Option<String>,
    /// The stream path, under its alternate protocol name.
    pub path: Option<String>,
    /// The acknowledged offset.
    pub offset: String,
}

impl AckEntry {
    /// The stream path, from `stream` or `path`.
    #[must_use]
    pub fn stream_path(&self) -> Option<&str> {
        self.stream.as_deref().or(self.path.as_deref())
    }
}

/// The ack, callback and release request body.
#[derive(Clone, Debug, Deserialize)]
pub struct AckRequest {
    /// The wake identifier.
    pub wake_id: Option<String>,
    /// The subscription generation.
    pub generation: Option<u64>,
    /// The acknowledged offsets.
    pub acks: Option<Vec<AckEntry>>,
    /// True when the worker finished the wake.
    #[serde(default)]
    pub done: bool,
}
