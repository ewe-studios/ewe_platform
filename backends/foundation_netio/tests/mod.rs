mod event_source;
mod http2;
mod http3;
mod http_stream;
mod native;
mod netcap;
// `RawStream` exists only on native targets, so this suite is native-only.
#[cfg(not(target_family = "wasm"))]
mod read_wake_source_tests;
mod simple_http;
mod tcp_and_tls;
mod websocket;
mod webtransport;
