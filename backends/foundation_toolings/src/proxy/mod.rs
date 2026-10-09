pub mod handlers;
pub mod tunnel_proxy;

pub use handlers::*;
pub use tunnel_proxy::*;

// -- Reloader.js embedded asset

/// The SSE reloader JavaScript, embedded at compile time.
pub static RELOADER_JS: &[u8] = include_bytes!("reloader.js");
