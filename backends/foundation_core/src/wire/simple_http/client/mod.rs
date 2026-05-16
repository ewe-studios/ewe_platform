// HTTP 1.1 Client Module

// Shared types — always compiled, including on wasm32
pub mod shared;
pub use shared::*;

// Native types — gated behind not(target_arch = "wasm32")
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

// Re-export submodules for backward compatibility: client::body_reader, client::redirects, etc.
// These were previously top-level modules of client/ and are now in shared/.
pub use shared::body_reader;
pub use shared::redirects;
pub use shared::dns;
pub use shared::cookie;
pub use shared::control;
pub use shared::compression;
pub use shared::intro;
pub use shared::middleware;
pub use shared::proxy;
pub use shared::config;
pub use shared::request as request_module;

// Re-export Extensions from simple_http::shared
pub use crate::wire::simple_http::shared::Extensions;

// Re-export ExecutionAction from valtron
pub use crate::valtron::ExecutionAction;
