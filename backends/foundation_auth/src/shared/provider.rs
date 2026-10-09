//! Upstream identity provider entity for social login.
//!
//! WHY: the IdP brokers authentication through upstream providers (Google,
//! GitHub, Facebook, ...). Each configured provider is stored as an
//! `UpstreamProvider` row; its client secret is encrypted at rest.
//!
//! WHAT: `UpstreamProvider` (config), `ProviderType` (OIDC vs OAuth2), and
//! `ProviderMapping` (claim → user-field mapping rules).
//!
//! HOW: plain serde structs. Persistence + encryption live in
//! `services::provider_service`. `scopes` and `mapping_config` are stored as
//! JSON strings in SQL and parsed on read.

use serde::{Deserialize, Serialize};

/// How the IdP talks to an upstream provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    /// Full OIDC: discovery document + userinfo endpoint + ID token.
    Oidc,
    /// OAuth2 only: explicit authorize/token/userinfo URLs, no ID token.
    Oauth2,
}

impl ProviderType {
    /// Wire representation stored in the `provider_type` column.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Oidc => "oidc",
            Self::Oauth2 => "oauth2",
        }
    }

    /// Parse from the stored column value. Unknown values default to `Oidc`.
    #[must_use]
    pub fn from_str_lenient(s: &str) -> Self {
        match s {
            "oauth2" => Self::Oauth2,
            _ => Self::Oidc,
        }
    }
}

impl core::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Maps upstream profile claim names to our normalized `User` fields.
///
/// Defaults follow OIDC standard claim names. OAuth2-only providers (e.g.
/// GitHub uses `id`/`login`) override the relevant fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMapping {
    /// Claim holding the email address. Default: `"email"`.
    pub email_field: String,
    /// Claim holding email-verified status. Default: `"email_verified"`.
    pub email_verified_field: String,
    /// Claim holding the display name. Default: `"name"`.
    pub name_field: String,
    /// Claim holding the stable subject id. Default: `"sub"` (OIDC) / `"id"` (OAuth2).
    pub subject_field: String,
    /// Claim holding the preferred username. Default: `"preferred_username"`.
    pub username_field: Option<String>,
}

impl Default for ProviderMapping {
    fn default() -> Self {
        Self {
            email_field: "email".into(),
            email_verified_field: "email_verified".into(),
            name_field: "name".into(),
            subject_field: "sub".into(),
            username_field: Some("preferred_username".into()),
        }
    }
}

/// A configured upstream identity provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamProvider {
    /// Slug id: `"google"`, `"github"`, or a custom label.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// OIDC or OAuth2.
    pub provider_type: ProviderType,
    /// Upstream-issued client id (public).
    pub client_id: String,
    /// Encrypted client secret: `nonce || ciphertext`. `None` until set.
    pub client_secret_ciphertext: Option<Vec<u8>>,
    /// Identifies the key used to encrypt `client_secret_ciphertext` (for rotation).
    pub encryption_key_id: String,
    /// Authorization endpoint (required when there's no `discovery_url`).
    pub authorization_url: Option<String>,
    /// Token endpoint (required when there's no `discovery_url`).
    pub token_url: Option<String>,
    /// Userinfo endpoint (OAuth2 providers without OIDC discovery).
    pub userinfo_url: Option<String>,
    /// OIDC discovery `.well-known` URL. When set, endpoints are discovered.
    pub discovery_url: Option<String>,
    /// Requested scopes, e.g. `["openid", "email", "profile"]`.
    pub scopes: Vec<String>,
    /// Whether this provider is offered for login.
    pub is_active: bool,
    /// Claim → user-field mapping rules.
    pub mapping_config: ProviderMapping,
    /// Creation time (epoch millis).
    pub created_at: i64,
    /// Last update time (epoch millis).
    pub updated_at: i64,
}

impl UpstreamProvider {
    /// Whether the provider has enough config to build an authorize URL:
    /// either an OIDC discovery URL, or explicit authorization + token URLs.
    #[must_use]
    pub fn has_endpoints(&self) -> bool {
        self.discovery_url.is_some()
            || (self.authorization_url.is_some() && self.token_url.is_some())
    }
}

/// Fields that can be updated on an existing provider. `None` leaves the
/// current value unchanged. The client secret is updated separately via
/// `ProviderService::set_secret` (it needs encryption).
#[derive(Debug, Clone, Default)]
pub struct ProviderUpdate {
    pub name: Option<String>,
    pub provider_type: Option<ProviderType>,
    pub client_id: Option<String>,
    pub authorization_url: Option<Option<String>>,
    pub token_url: Option<Option<String>>,
    pub userinfo_url: Option<Option<String>>,
    pub discovery_url: Option<Option<String>>,
    pub scopes: Option<Vec<String>>,
    pub is_active: Option<bool>,
    pub mapping_config: Option<ProviderMapping>,
}
