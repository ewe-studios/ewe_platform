//! Shared client types — always compiled, including on wasm32.

pub mod body_reader;
pub mod compression;
pub mod config;
pub mod control;
pub mod cookie;
pub mod dns;
pub mod intro;
pub mod middleware;
pub mod proxy;
pub mod redirects;
pub mod request;

pub use config::ClientConfig;
pub use request::PreparedRequest;
pub use body_reader::*;
pub use compression::*;
pub use control::*;
pub use cookie::*;
pub use dns::*;
pub use intro::*;
pub use middleware::*;
pub use proxy::*;
pub use redirects::*;
