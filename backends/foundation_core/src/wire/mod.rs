// Wire modules depend on netcap::RawStream, Connection, SSLConnector — only available on native targets.
// On wasm32, these are excluded unless you have alternative netcap bindings (use `wire-native` feature
// with your own netcap wasm implementation).
#[cfg(not(target_arch = "wasm32"))]
pub mod event_source;

#[cfg(not(target_arch = "wasm32"))]
pub mod http_stream;

#[cfg(not(target_arch = "wasm32"))]
pub mod simple_http;

#[cfg(not(target_arch = "wasm32"))]
pub mod websocket;
