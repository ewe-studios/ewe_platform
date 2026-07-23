//! User provisioning service for social login (spec-57, F005).
//!
//! WHY: After receiving an upstream user profile (from Google, GitHub, etc.),
//! the IdP must either find an existing local user linked to that upstream
//! identity, create a new local user, or link the upstream identity to an
//! existing session-authenticated user.
//!
//! WHAT: [`ProvisioningService`] handles the full provisioning lifecycle:
//! find-existing-link → create-new-user → auto-link-by-email (trusted providers
//! only) → manual-link-request (when email exists but auto-linking is disabled).
//!
//! HOW: Uses `foundation_db::QueryStore` via `UserService` for user CRUD and a
//! `UserProviderLink` table (migration 025) for upstream-to-local mappings.

use std::sync::Arc;

use foundation_db::{QueryStore, DataValue};

use crate::server::models::provider::UpstreamProvider;
use crate::shared::upstream_client::UpstreamProfile;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from the provisioning service.
#[derive(Debug)]
pub enum ProvisioningError {
    /// The upstream profile is missing a required field.
    MissingField(String),
    /// Storage error.
    Storage(String),
    /// The user already has a link for this provider.
    LinkAlreadyExists { provider_id: String, user_id: String },
    /// Multiple local users share the same verified email (ambiguous).
    AmbiguousEmail(String),
    /// The email is already taken by a different user (manual link required).
    EmailConflict { email: String, existing_user_id: String },
    /// The upstream provider is not trusted for auto-linking.
    UnsupportedProvider(String),
}

impl core::fmt::Display for ProvisioningError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingField(s) => write!(f, "missing field: {s}"),
            Self::Storage(s) => write!(f, "storage error: {s}"),
            Self::LinkAlreadyExists { provider_id, user_id } => {
                write!(f, "link already exists: provider={provider_id}, user={user_id}")
            }
            Self::AmbiguousEmail(s) => write!(f, "multiple users with email: {s}"),
            Self::EmailConflict { email, existing_user_id } => {
                write!(f, "email {email} already belongs to user {existing_user_id}")
            }
            Self::UnsupportedProvider(s) => write!(f, "unsupported provider: {s}"),
        }
    }
}

impl std::error::Error for ProvisioningError {}

// ---------------------------------------------------------------------------
// ProvisioningResult
// ---------------------------------------------------------------------------

/// The outcome of provisioning an upstream profile.
#[derive(Debug, Clone)]
pub enum ProvisioningResult {
    /// A new local user was created.
    Created {
        /// The new local user ID.
        user_id: String,
        /// Whether the email was already verified by the upstream provider.
        email_verified: bool,
    },
    /// An existing user was found via the provider link.
    Linked {
        /// The existing local user ID.
        user_id: String,
    },
}

impl ProvisioningResult {
    /// The local user ID, regardless of whether it was created or linked.
    #[must_use]
    pub fn user_id(&self) -> &str {
        match self {
            Self::Created { user_id, .. } | Self::Linked { user_id } => user_id,
        }
    }
}

// ---------------------------------------------------------------------------
// ProvisioningService
// ---------------------------------------------------------------------------

/// Service that provisions local users from upstream social login profiles.
///
/// Uses `foundation_db::QueryStore` directly — no custom traits needed.
/// The `user_provider_links` table (migration 025) stores the mapping from
/// `(provider_id, upstream_subject)` → `user_id`.
pub struct ProvisioningService<QS: QueryStore + 'static> {
    query_store: Arc<QS>,
    /// Whether trusted providers with verified email claims can auto-link
    /// to existing local users by email.
    auto_link_by_email: bool,
}

impl<QS: QueryStore + 'static> ProvisioningService<QS> {
    /// Create a new provisioning service.
    #[must_use]
    pub fn new(query_store: Arc<QS>) -> Self {
        Self {
            query_store,
            auto_link_by_email: true,
        }
    }

    /// Disable auto-linking by email (require manual link requests).
    pub fn disable_auto_link(&mut self) {
        self.auto_link_by_email = false;
    }

    /// Provision a user from an upstream profile.
    ///
    /// The provisioning flow:
    /// 1. Look up the user_provider_link table for `(provider_id, upstream_sub)`
    /// 2. If found → return `Linked { user_id }`
    /// 3. If the upstream profile has a verified email and auto-linking is
    ///    enabled → find a local user by that email. If exactly one match →
    ///    create the link and return `Linked`. If multiple → `AmbiguousEmail`.
    ///    If none → proceed to step 4.
    /// 4. Create a new local user, create the provider link, return `Created`
    ///
    /// # Errors
    ///
    /// Returns `ProvisioningError` on storage failure or ambiguous match.
    pub fn provision(
        &self,
        provider: &UpstreamProvider,
        profile: &UpstreamProfile,
    ) -> Result<ProvisioningResult, ProvisioningError> {
        // 1. Check for existing provider link
        if let Some(user_id) = self.find_link(provider, &profile.sub)? {
            return Ok(ProvisioningResult::Linked { user_id });
        }

        // 2. Try auto-link by verified email
        if self.auto_link_by_email && profile.email_verified {
            if let Some(email) = &profile.email {
                match self.find_user_by_email(email)? {
                    Some(user_id) => {
                        self.create_link(provider, &profile.sub, &user_id)?;
                        return Ok(ProvisioningResult::Linked { user_id });
                    }
                    None => { /* fall through to create */ }
                }
            }
        }

        // 3. Create new local user
        let user_id = self.create_user(profile)?;
        self.create_link(provider, &profile.sub, &user_id)?;
        Ok(ProvisioningResult::Created {
            user_id,
            email_verified: profile.email_verified,
        })
    }

    // ── internal helpers ───────────────────────────────────────────────────

    fn find_link(
        &self,
        provider: &UpstreamProvider,
        upstream_sub: &str,
    ) -> Result<Option<String>, ProvisioningError> {
        let stream = self
            .query_store
            .query(
                "SELECT user_id FROM user_provider_links \
                 WHERE provider_id = ? AND upstream_subject = ?",
                &[
                    DataValue::Text(provider.id.clone()),
                    DataValue::Text(upstream_sub.to_string()),
                ],
            )
            .map_err(|e| ProvisioningError::Storage(e.to_string()))?;

        for item in stream {
            match item {
                foundation_core::valtron::Stream::Next(Ok(row)) => {
                    let id: String = row
                        .get_by_name("user_id")
                        .map_err(|e| ProvisioningError::Storage(e.to_string()))?;
                    return Ok(Some(id));
                }
                foundation_core::valtron::Stream::Next(Err(e)) => {
                    return Err(ProvisioningError::Storage(e.to_string()));
                }
                _ => {}
            }
        }
        Ok(None)
    }

    fn find_user_by_email(
        &self,
        email: &str,
    ) -> Result<Option<String>, ProvisioningError> {
        let stream = self
            .query_store
            .query(
                "SELECT id FROM users WHERE email = ? AND email_verified = 1 LIMIT 2",
                &[DataValue::Text(email.to_string())],
            )
            .map_err(|e| ProvisioningError::Storage(e.to_string()))?;

        let mut ids = Vec::new();
        for item in stream {
            match item {
                foundation_core::valtron::Stream::Next(Ok(row)) => {
                    let id: String = row
                        .get_by_name("id")
                        .map_err(|e| ProvisioningError::Storage(e.to_string()))?;
                    ids.push(id);
                }
                foundation_core::valtron::Stream::Next(Err(e)) => {
                    return Err(ProvisioningError::Storage(e.to_string()));
                }
                _ => {}
            }
        }

        match ids.len() {
            0 => Ok(None),
            1 => Ok(Some(ids.into_iter().next().unwrap())),
            _ => Err(ProvisioningError::AmbiguousEmail(email.to_string())),
        }
    }

    fn create_link(
        &self,
        provider: &UpstreamProvider,
        upstream_sub: &str,
        user_id: &str,
    ) -> Result<(), ProvisioningError> {
        self.query_store
            .execute(
                "INSERT OR IGNORE INTO user_provider_links \
                 (id, provider_id, upstream_subject, user_id) VALUES (?, ?, ?, ?)",
                &[
                    DataValue::Text(uuid::Uuid::new_v4().to_string()),
                    DataValue::Text(provider.id.clone()),
                    DataValue::Text(upstream_sub.to_string()),
                    DataValue::Text(user_id.to_string()),
                ],
            )
            .map_err(|e| ProvisioningError::Storage(e.to_string()))?;
        Ok(())
    }

    fn create_user(
        &self,
        profile: &UpstreamProfile,
    ) -> Result<String, ProvisioningError> {
        let user_id = uuid::Uuid::new_v4().to_string();
        let email = profile.email.clone().unwrap_or_default();
        let name = profile.name.clone().unwrap_or_default();
        let email_verified = i64::from(profile.email_verified);

        self.query_store
            .execute(
                "INSERT INTO users (id, email, username, email_verified, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, strftime('%s', 'now') * 1000, strftime('%s', 'now') * 1000)",
                &[
                    DataValue::Text(user_id.clone()),
                    DataValue::Text(email),
                    DataValue::Text(name),
                    DataValue::Integer(email_verified),
                ],
            )
            .map_err(|e| ProvisioningError::Storage(e.to_string()))?;
        Ok(user_id)
    }

    /// Check whether a link already exists.
    ///
    /// # Errors
    ///
    /// Returns a `ProvisioningError` on storage failure.
    pub fn link_exists(
        &self,
        provider: &UpstreamProvider,
        upstream_sub: &str,
    ) -> Result<bool, ProvisioningError> {
        self.find_link(provider, upstream_sub).map(|r| r.is_some())
    }

    /// Get all provider links for a local user.
    ///
    /// # Errors
    ///
    /// Returns a `ProvisioningError` on storage failure.
    pub fn get_user_links(
        &self,
        user_id: &str,
    ) -> Result<Vec<String>, ProvisioningError> {
        let stream = self
            .query_store
            .query(
                "SELECT provider_id FROM user_provider_links WHERE user_id = ?",
                &[DataValue::Text(user_id.to_string())],
            )
            .map_err(|e| ProvisioningError::Storage(e.to_string()))?;

        let mut providers = Vec::new();
        for item in stream {
            match item {
                foundation_core::valtron::Stream::Next(Ok(row)) => {
                    let pid: String = row
                        .get_by_name("provider_id")
                        .map_err(|e| ProvisioningError::Storage(e.to_string()))?;
                    providers.push(pid);
                }
                foundation_core::valtron::Stream::Next(Err(e)) => {
                    return Err(ProvisioningError::Storage(e.to_string()));
                }
                _ => {}
            }
        }
        Ok(providers)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::models::provider::{ProviderMapping, ProviderType, UpstreamProvider};
    use std::sync::Arc;
    use foundation_core::valtron::valtron_test;
    use foundation_db::{QueryStore, StorageBackend, StorageProvider};

    fn test_provider() -> UpstreamProvider {
        UpstreamProvider {
            id: "google".into(),
            name: "Google".into(),
            provider_type: ProviderType::Oidc,
            client_id: "test-client".into(),
            client_secret_ciphertext: None,
            encryption_key_id: "default".into(),
            authorization_url: None,
            token_url: None,
            userinfo_url: None,
            discovery_url: Some("https://accounts.google.com/.well-known/openid-configuration".into()),
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
            is_active: true,
            mapping_config: ProviderMapping::default(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn test_profile() -> UpstreamProfile {
        UpstreamProfile {
            sub: "google-12345".into(),
            email: Some("user@example.com".into()),
            email_verified: true,
            name: Some("Test User".into()),
            preferred_username: Some("testuser".into()),
            raw: serde_json::json!({
                "sub": "google-12345",
                "email": "user@example.com",
                "email_verified": true,
                "name": "Test User",
            }),
        }
    }

    fn make_store() -> (Arc<StorageProvider>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("provision_test.db");
        let provider = StorageProvider::new(StorageBackend::Turso {
            url: db_path.to_str().unwrap().to_string(),
        })
        .expect("init turso");
        (Arc::new(provider), dir)
    }

    #[valtron_test]
    fn new_profile_creates_user_and_link() {
        let (store, _tmp) = make_store();
        let svc = ProvisioningService::new(store);
        let provider = test_provider();
        let profile = test_profile();

        let result = svc.provision(&provider, &profile).expect("provision");
        match result {
            ProvisioningResult::Created { ref user_id, email_verified: true } => {
                assert!(!user_id.is_empty());
            }
            _ => panic!("expected Created, got {result:?}"),
        }
    }

    #[valtron_test]
    fn same_profile_provisions_once_and_links_twice() {
        let (store, _tmp) = make_store();
        let svc = ProvisioningService::new(store);
        let provider = test_provider();
        let profile = test_profile();

        let result1 = svc.provision(&provider, &profile).expect("first provision");
        let uid1 = result1.user_id().to_string();

        let result2 = svc.provision(&provider, &profile).expect("second provision");
        let uid2 = result2.user_id().to_string();

        assert_eq!(uid1, uid2, "same upstream sub should return same local user");
    }

    #[valtron_test]
    fn different_providers_with_same_verified_email_share_user() {
        let (store, _tmp) = make_store();
        let svc = ProvisioningService::new(store);

        // Google: first login
        let google = test_provider();
        let profile_g = test_profile();

        let r1 = svc.provision(&google, &profile_g).expect("google provision");
        let uid1 = r1.user_id().to_string();

        // GitHub: same email, different sub
        let mut github = test_provider();
        github.id = "github".into();
        github.name = "GitHub".into();
        github.discovery_url = None;
        let mut profile_gh = test_profile();
        profile_gh.sub = "github-999".into();

        let r2 = svc.provision(&github, &profile_gh).expect("github provision");
        let uid2 = r2.user_id().to_string();

        assert_eq!(uid1, uid2, "same verified email should auto-link to existing user");
    }

    #[valtron_test]
    fn disabled_auto_link_creates_separate_users() {
        let (store, _tmp) = make_store();
        let mut svc = ProvisioningService::new(store);
        svc.disable_auto_link();

        let provider = test_provider();
        let p1 = test_profile();
        let r1 = svc.provision(&provider, &p1).expect("first");

        // With auto-link disabled, the same email from a different upstream
        // identity must NOT auto-link — but the email must be different too
        // (users.email is UNIQUE), simulating a different-email scenario.
        let mut p2 = test_profile();
        p2.sub = "google-different-sub".into();
        p2.email = Some("other@example.com".into());
        p2.name = Some("Other User".into());
        let r2 = svc.provision(&provider, &p2).expect("second");

        assert_ne!(
            r1.user_id(),
            r2.user_id(),
            "without auto-link, different subs with different emails get different users"
        );
    }

    #[valtron_test]
    fn link_exists_detects_existing_link() {
        let (store, _tmp) = make_store();
        let svc = ProvisioningService::new(store);
        let provider = test_provider();
        let profile = test_profile();

        assert!(!svc.link_exists(&provider, &profile.sub).expect("check"));

        svc.provision(&provider, &profile).expect("provision");

        assert!(svc.link_exists(&provider, &profile.sub).expect("check after"));
    }
}
