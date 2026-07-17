---
feature: "05-user-provisioning"
spec: "58-foundation-auth-social-login"
depends: "01-provider-model, 02-provider-migrations, 04-social-login-flow"
status: "pending"
---

# Feature 05: User Provisioning from Social Login

## Goal

After receiving an upstream user profile, create a new local user or link to an existing one.

## WHAT

### ProvisioningService

```rust
pub struct ProvisioningService<QS: QueryStore + 'static> {
    query_store: Arc<QS>,
}

impl ProvisioningService {
    /// Main entry point called after upstream profile is received.
    /// Returns the local user ID that should be authenticated.
    pub async fn provision(
        &self,
        provider: &UpstreamProvider,
        profile: &ProviderProfile,
    ) -> Result<ProvisioningResult, Error>;
}

pub enum ProvisioningResult {
    /// New user created from upstream profile
    Created { user_id: String },
    /// Linked to existing user by email match
    Linked { user_id: String },
    /// Already linked — user exists with this provider link
    AlreadyLinked { user_id: String },
}
```

### Provisioning Logic (D05)

```
1. Check if user_provider_links already has (provider_id, upstream_subject)
   → AlreadyLinked: return existing user_id

2. Check if upstream email is verified AND a user exists with that email
   → Linked: create user_provider_links row, return existing user_id

3. Otherwise: create new user + user_provider_links row
   → Created: return new user_id
```

### New User Creation

When creating a new user from an upstream profile:

```rust
User {
    id: uuid::Uuid::new_v4().to_string(),
    email: profile.email.clone().unwrap_or_default(),
    username: profile.username.clone(),
    password_hash: None,          // no password for social-only users
    email_verified: profile.email_verified,
    email_verified_at: if profile.email_verified { Some(now) } else { None },
    created_at: now,
    updated_at: now,
    metadata: Some(json!({
        "source": "social",
        "provider": provider.id,
        "upstream_subject": profile.subject,
    })),
    failed_login_attempts: 0,
    locked_until: None,
    deleted_at: None,
}
```

### UserProviderLink Creation

```rust
UserProviderLink {
    id: uuid::Uuid::new_v4().to_string(),
    user_id: the_user.id.clone(),
    provider_id: provider.id.clone(),
    upstream_subject: profile.subject.clone(),
    upstream_email: profile.email.clone(),
    upstream_username: profile.username.clone(),
    created_at: now,
    last_used_at: None,  // set on first use
}
```

### Update last_used_at

On every social login (including AlreadyLinked), update `last_used_at` on the link row. This enables "last used provider" UI and cleanup of stale links.

## HOW

### Files to create

- `backends/foundation_auth/src/server/services/provisioning_service.rs` — ProvisioningService
- `backends/foundation_auth/src/server/models/provider_link.rs` — UserProviderLink entity

### Files to modify

- `backends/foundation_auth/src/server/handlers/core.rs` — call ProvisioningService in `provider_callback`
- `backends/foundation_auth/src/server/storage.rs` — add `find_user_by_email`, `create_user`, `create_provider_link`, `find_provider_link`, `update_provider_last_used` functions

### Integration Point

Called from `provider_callback` after `fetch_userinfo` succeeds and before creating the IdP authorization code:

```rust
let profile = upstream_client.fetch_userinfo(&provider, &tokens.access_token).await?;
let result = provisioning.provision(&provider, &profile).await?;

// Now create IdP auth code for this user_id
let user = find_user_by_id(storage.query_store.as_ref(), &result.user_id())?;
let auth_code = AuthorizationCode::new(&user, &client, ...);
```

---

_Created: 2026-07-17_
