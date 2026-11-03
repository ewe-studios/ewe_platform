//! Terms of Service model — versioned `ToS` with acceptance tracking.

use serde::{Deserialize, Serialize};

/// A version of the Terms of Service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TosVersion {
    pub version: String,
    pub content: String,
    pub created_at: i64,
    pub effective_from: i64,
}

/// A user's acceptance of a `ToS` version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TosAcceptance {
    pub user_id: String,
    pub tos_version: String,
    pub accepted_at: i64,
    pub ip_address: Option<String>,
}

/// What the server needs to finish a login after the user accepts the `ToS`.
///
/// Held inside a stored `ToS` code. It never holds a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingLogin {
    /// The method that proved the user: `password`, `totp`, `webauthn`,
    /// `passkey` or `social`.
    pub auth_method: String,
    /// The OAuth client the user logs in to, if the request named one.
    pub client_id: Option<String>,
    /// The redirect URI, already checked against the client.
    pub redirect_uri: Option<String>,
    /// The application `state` value to give back to the client (social login).
    pub state: Option<String>,
    /// The `OpenID` `nonce` for the authorization code (social login).
    pub nonce: Option<String>,
}

/// The record stored for one `ToS` code, under the SHA-256 of the code.
///
/// The code itself is never stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TosCodeRecord {
    /// The user the code was issued for.
    pub user_id: String,
    /// The `ToS` version the user must accept with this code.
    pub tos_version: String,
    /// What is needed to finish the login.
    pub pending_login: PendingLogin,
    /// Unix time in milliseconds after which the code is no longer valid.
    pub expires_at: i64,
}
