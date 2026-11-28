//! The native Hypertext Transfer Protocol (HTTP) surface.
//!
//! The control endpoints are [`Serve`](foundation_http::native::serve::Serve) handlers. A host
//! mounts them beside its Durable Streams handler.

pub mod handler;
pub mod webhook;

pub use handler::SubscriptionHandler;
pub use webhook::NetioWebhookSender;
