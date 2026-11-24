//! HTTP client module — canonical home for the unified HTTP client (F51 Stage 4).

mod api;
mod connection;
mod connector;
mod http_client_impl;
mod pool;
mod proxy;
mod request;
mod tasks;
/// The TLS handshake task of a build with a TLS provider.
#[cfg(any(
    feature = "ssl-rustls",
    feature = "ssl-openssl",
    feature = "ssl-native-tls"
))]
mod tls_task;

pub use api::*;
pub use connection::*;
pub use connector::Connector;
pub use http_client_impl::*;
pub use pool::*;
pub use proxy::*;
pub use request::*;
pub use tasks::*;
#[cfg(any(
    feature = "ssl-rustls",
    feature = "ssl-openssl",
    feature = "ssl-native-tls"
))]
pub use tls_task::*;
