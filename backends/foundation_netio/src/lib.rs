// wire modules depend on netcap::rawstream, connection, sslconnector — only available on native targets.
// on wasm32, these are excluded unless you have alternative netcap bindings (use `wire-native` feature
// with your own netcap wasm implementation).

// event_source and simple_http have native-only client code but shared types are wasm-compatible.
// their modules internally gate native parts behind cfg(not(target_family = "wasm")).

#![cfg_attr(feature = "nightly", feature(unix_socket_peek, tcp_linger))]

pub mod event_source;
pub mod simple_http;
pub mod websocket;
pub mod netcap;

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub mod http_stream;

#[cfg(all(feature = "ssl-native-tls", not(target_family = "wasm")))]
extern crate native_tls;
