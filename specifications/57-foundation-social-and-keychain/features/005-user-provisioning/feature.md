# Feature 05: User Provisioning from Social Login

**Status:** Complete

**Depends on:** `01-provider-model`, `02-provider-migrations`, `04-social-login-flow`

## Summary

After receiving an upstream user profile, creates a new local user or links to an existing one. Implements the three-way provisioning decision: already linked (return existing user), email match (link to existing account), or new user creation.

## What was built

### Files created

- `backends/foundation_auth/src/server/services/provisioning_service.rs` -- `ProvisioningService`, `ProvisioningResult`, `ProvisioningError`

### Files modified

- `backends/foundation_auth/src/server/handlers/core.rs` -- integration point in `provider_callback`

### Key types

| Type | Purpose |
|------|---------|
| `ProvisioningService<QS: QueryStore>` | Main service: `provision(provider, profile) -> ProvisioningResult` |
| `ProvisioningResult` | Enum: `Created { user_id }`, `Linked { user_id }`, `AlreadyLinked { user_id }` |
| `ProvisioningError` | Error enum covering DB failures, duplicate checks, and missing data |

### Provisioning decision tree (D05)

1. **Check for existing link:** `SELECT user_id FROM user_provider_links WHERE provider_id = ? AND upstream_subject = ?` -> if found, return `AlreadyLinked { user_id }` and update `last_used_at`
2. **Email match (auto-link):** If upstream email is verified AND a local user exists with that email -> create `user_provider_links` row, return `Linked { user_id }`
3. **New user:** Create a new `User` with `password_hash: None` (social-only user), create `user_provider_links` row, return `Created { user_id }`

### New user fields

- `metadata.source = "social"`, `metadata.provider = provider.id`
- `email_verified` set from upstream profile (not default-false)
- `password_hash: None` -- social-only users have no password

## Tests

- `backends/foundation_auth/src/server/services/provisioning_service.rs` (inline `#[cfg(test)]` module) -- **5 valtron_test tests**
  - New user creation from upstream profile
  - AlreadyLinked: existing provider link returns same user
  - Email match: verified email links to existing account
  - Email not verified: falls through to new user creation
  - last_used_at updated on repeat login

## Related decisions

- `decisions/00-identity-broker-pattern.md` -- D05 provisioning + linking strategy
- `decisions/02-storage-abstraction.md` -- `QueryStore` trait for DB access

## Implementation notes

- Tests require the valtron test framework (`#[valtron_test]` with `FairGate` serialization) -- not plain `#[test]`
- `ProvisioningService` depends on `QueryStore`, not a concrete DB handle, keeping it testable without a real database in unit tests
- **Auto-link by email requires `email_verified: true`** from the upstream provider -- unverified emails never auto-link (security boundary)
- `last_used_at` is updated on every social login (including `AlreadyLinked`) to enable "last used provider" UI and stale-link cleanup
- Social-only users (no password) rely entirely on the upstream provider for authentication -- if the provider link is deleted, the user cannot log in
