//! Todo views, actions, and host entry points.
pub mod shared;
pub use shared::*;

#[cfg(all(feature = "server", not(target_family = "wasm")))]
pub mod server;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod web;

#[cfg(all(feature = "mobile", not(target_family = "wasm")))]
mod mobile;

#[cfg(all(feature = "mobile", not(target_family = "wasm")))]
pub use mobile::*;

#[cfg(all(feature = "mobile", not(target_family = "wasm")))]
uniffi::setup_scaffolding!();

#[cfg(all(
    any(feature = "server", feature = "mobile"),
    not(target_family = "wasm")
))]
#[allow(clippy::all, clippy::pedantic, missing_docs, unused_qualifications)]
pub mod todo_rpc {
    include!(concat!(env!("OUT_DIR"), "/ewe.todo.v1.mod.rs"));
    include!(concat!(env!("OUT_DIR"), "/todo_service.rs"));
}
