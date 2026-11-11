//! Bitwarden API route handlers (stubs — spec-57, F008).
//!
//! Full handler implementations live in F009 (Cloudflare) and F010 (native).
//! This module declares the handler signatures that both backends implement.

/// Accounts API — prelogin, register, password hint, profile, keys, avatar.
pub mod accounts;
/// Identity API — connect/token (login + refresh).
pub mod identity;

/// Ciphers API — CRUD, share, move, purge, restore.
pub mod ciphers;
/// Emergency access API.
pub mod emergency;
/// Events API — audit log.
pub mod events;
/// Folders API — CRUD.
pub mod folders;
/// Icon proxy API.
pub mod icons;
/// Organizations API — CRUD + membership.
pub mod orgs;
/// Sends API — CRUD + anonymous access.
pub mod sends;
/// Sync API — full vault sync.
pub mod sync;
/// Two-factor API — setup, verify, recovery codes.
pub mod two_factor;
