# Feature 01: Provider Model + CRUD Service

**Status:** Complete

**Depends on:** `decisions/00-identity-broker-pattern.md`

## Summary

Defines the `UpstreamProvider` entity, `ProviderType` enum, `ProviderMapping` claim rules, and a `ProviderService` with full CRUD. All other social-login features depend on this data model. Provider secrets are encrypted at rest using ChaCha20-Poly1305 with an Argon2id-derived key.

## What was built

### Files created

- `backends/foundation_auth/src/server/models/provider.rs` -- `UpstreamProvider`, `ProviderType`, `ProviderMapping`, `ProviderUpdate`
- `backends/foundation_auth/src/server/services/provider_service.rs` -- `ProviderService`, `ProviderCrypto`

### Key types

| Type | Purpose |
|------|---------|
| `UpstreamProvider` | Full provider config (id, name, client credentials, endpoints, scopes, mapping) |
| `ProviderType` | Enum: `Oidc` (full discovery) or `Oauth2` (custom URLs) |
| `ProviderMapping` | Claim-name normalization (email_field, name_field, subject_field, etc.) |
| `ProviderUpdate` | Partial-update struct for PATCH/PUT semantics |
| `ProviderService` | CRUD: `create`, `find_by_id`, `find_active`, `update`, `delete`, `set_secret`, `get_secret` |
| `ProviderCrypto` | Encrypt/decrypt using `PROVIDER_SECRET_KEY` env var via Argon2id key derivation |

### Dependencies

- `chacha20poly1305` (authenticated encryption)
- `argon2` (key derivation)

## Tests

- `tests/integration/provider_service/mod.rs` -- **18 integration tests**
  - Create, find, update, delete, set_secret/get_secret round-trips
  - Validation (empty id, missing client_id, no endpoints configured)
  - `find_active` filtering
  - Secret encryption idempotency

## Related decisions

- `decisions/00-identity-broker-pattern.md` -- upstream provider abstraction + claim normalization

## Implementation notes

- Validation on `create`: id non-empty, client_id non-empty, at least one of `discovery_url` or `(authorization_url + token_url)` set
- `get_secret` decrypts and returns plaintext -- only used during auth flow, never logged
- Provider secrets are stored as `BLOB` in SQLite (`nonce || ciphertext`)
- All tests use the valtron test framework; `#[valtron_test]` with `FairGate` for serialization
