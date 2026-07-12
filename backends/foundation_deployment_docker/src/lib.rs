//! Docker Engine API v1.53 client — bollard-free, over `DynNetClient` + valtron.
//!
//! WHY: Spec 53 proved bollard is intertwined with tokio (transport, streaming
//! decoders, BuildKit gRPC). This crate talks to the Docker daemon via our own
//! cross-platform HTTP surface (`foundation_netio::DynNetClient` +
//! `PreparedRequestBuilder`) over a Unix socket, driven by valtron async tasks.
//!
//! WHAT: Auto-generated API client functions live in `generated/` (run
//! `cargo run --bin genapi -- generate docker` to regenerate). Hand-written
//! Docker-specific glue (`DockerClient`, streaming endpoints, `Deployable`
//! impls, `LogFrameDecoder`) lives alongside.
//!
//! HOW: The generator emits `async fn` per endpoint that call
//! `HttpClient::send_async()`; the client is built with
//! `HttpClientBuilder::new().unix_socket("/var/run/docker.sock").build()`.

// Auto-generated API surface (types + async `*_request` fns). Feature-gated
// per the generated `generated/mod.rs` (`#![cfg(feature = "docker")]`).
pub mod generated;

// Hand-written Docker glue. Gated on `docker` because it calls the generated
// (feature-gated) `*_request` functions.
#[cfg(feature = "docker")]
pub mod client;
#[cfg(feature = "docker")]
pub mod error;

#[cfg(feature = "docker")]
pub use client::{DockerClient, DEFAULT_API_VERSION, DEFAULT_DOCKER_SOCKET};
#[cfg(feature = "docker")]
pub use error::DockerError;
