#![doc = include_str!("../README.md")]
#![cfg(not(target_family = "wasm"))]

extern crate self as foundation_ui_http;
mod action_route;
mod delivery;
mod events;
mod html;
#[cfg(feature = "live-stream")]
pub mod live_stream;
mod response;
mod socket;
pub use events::{DeliveryPrincipal, EventDelivery};
mod subscription;

pub use action_route::{ActionRegistryRouteHandler, ActionRequestPolicy};
pub use delivery::{serve_view, BodyWriterConfig};
pub use foundation_http as http;
pub use foundation_macros::ui_http_get as get;
pub use foundation_ui_runtime::view::template_types::Html;
pub use response::{
    FrameBody, IntoViewResponse, JsonBody, Response, ViewBody, ViewError, ViewRequest, ViewSession,
};
pub use socket::{ActionSocketRoute, SocketConfig};
