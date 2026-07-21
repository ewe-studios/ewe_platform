//! IdP server data models.

pub mod client;
pub mod code;
pub mod passkey;
pub mod token;
pub mod tos;
pub mod user;

// Provider models are cross-platform (pure serde) and live in `shared/provider.rs`
// so the upstream broker client can run on wasm/Workers too. Re-exported here to
// preserve the `server::models::provider::*` path used across the server module.
pub use crate::shared::provider;

pub use client::OAuthClient;
pub use code::{AuthorizationCode, DeviceCode};
pub use passkey::Passkey;
pub use provider::{ProviderMapping, ProviderType, ProviderUpdate, UpstreamProvider};
pub use token::RefreshToken;
pub use tos::{TosAcceptance, TosVersion};
pub use user::User;
