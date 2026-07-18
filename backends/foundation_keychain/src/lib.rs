//! `foundation_keychain` — Bitwarden-compatible vault + SSH key provisioning (spec-57).
//!
//! WHY: Provide a self-hosted, Bitwarden API-compatible secrets vault with
//! SSH key provisioning, built entirely on `foundation_db` (storage) and
//! `foundation_auth` (JWT/TOTP/PBKDF2/middleware). No custom traits, no
//! reinvented crypto, no tokio.
//!
//! WHAT: Bitwarden HTTP API (~150 routes), portable domain models, SignalR
//! MessagePack notifications, and SSH key generation + at-rest encryption.
//!
//! HOW: `core/` holds portable domain logic. `server/` holds platform-gated
//! HTTP entry points (Workers via foundation_deployment_cloudflare, native via
//! foundation_http). All storage through `foundation_db` traits; all auth
//! through `foundation_auth`.

pub mod core;

#[cfg(not(target_family = "wasm"))]
pub mod server;

// Core re-exports
pub use core::error::{AppError, AppResult};
pub use core::models::cipher::{
    Cipher, CipherCreateRequest, CipherType, CipherUpdateRequest, SecureNoteType,
};
pub use core::models::folder::{Folder, FolderCreateRequest, FolderUpdateRequest};
pub use core::models::org::{Organization, OrganizationCreateRequest, OrganizationUserType};
pub use core::models::send::{Send, SendCreateRequest, SendType, SendUpdateRequest};
pub use core::models::user::{KdfType, PreloginResponse, RegisterRequest, UserAccount};
pub use core::models::sync::SyncData;
