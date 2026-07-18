//! Bitwarden API route handlers (stubs — spec-57, F008).
//!
//! Full handler implementations live in F009 (Cloudflare) and F010 (native).
//! This module declares the handler signatures that both backends implement.

use crate::core::error::AppResult;

/// Accounts API — prelogin, register, password hint, profile, keys, avatar.
pub mod accounts;

/// Ciphers API — CRUD, share, move, purge, restore.
pub mod ciphers;
/// Folders API — CRUD.
pub mod folders;
/// Organizations API — CRUD + membership.
pub mod orgs;
/// Sends API — CRUD + anonymous access.
pub mod sends;
/// Two-factor API — setup, verify, recovery codes.
pub mod two_factor;
/// Sync API — full vault sync.
pub mod sync;
/// Events API — audit log.
pub mod events;
/// Emergency access API.
pub mod emergency;
/// Icon proxy API.
pub mod icons;
