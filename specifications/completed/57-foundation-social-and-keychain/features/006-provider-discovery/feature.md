# Feature 06: Provider Discovery + Admin API

**Status:** Complete

**Depends on:** `01-provider-model`, `02-provider-migrations`, `04-social-login-flow`

## Summary

Extends the IdP's OIDC discovery document to list available upstream providers, and adds a full admin CRUD API for managing providers at `/auth/v1/admin/providers`. Apps can dynamically discover which social login providers are configured (e.g. "We support Google and GitHub").

## What was built

### Files created / modified

- `backends/foundation_auth/src/server/handlers/provider_admin.rs` -- extended discovery (`providers_in_discovery`), admin CRUD handlers
- `backends/foundation_auth/src/server/handlers/mod.rs` -- re-exports
- `backends/foundation_auth/src/server/idp_server.rs` -- admin route registration
- `backends/foundation_auth/src/shared/discovery.rs` -- `providers` field added to discovery document

### Extended discovery

The `/.well-known/openid-configuration` response now includes:

```json
{
  "providers": [
    { "id": "google", "name": "Google", "type": "oidc", "icon_url": "/static/icons/google.svg" },
    { "id": "github", "name": "GitHub", "type": "oauth2", "icon_url": "/static/icons/github.svg" }
  ]
}
```

### Admin API endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/auth/v1/admin/providers` | List all providers (secrets redacted) |
| `POST` | `/auth/v1/admin/providers` | Create a new provider |
| `GET` | `/auth/v1/admin/providers/{id}` | Get one provider (secret redacted) |
| `PUT` | `/auth/v1/admin/providers/{id}` | Update provider config |
| `DELETE` | `/auth/v1/admin/providers/{id}` | Delete provider |
| `POST` | `/auth/v1/admin/providers/{id}/secret` | Set/update the provider secret |
| `POST` | `/auth/v1/admin/providers/{id}/test` | Test provider connection (discover + validate) |

### Secret handling

- `POST .../secret` accepts `{"secret": "GOCSPX-..."}` -- encrypted before storage via `ProviderCrypto`
- Secrets are **never returned** by any GET endpoint (always redacted)

### Test endpoint

Validates provider configuration without a full login flow:
1. Fetches discovery document (if `discovery_url` set)
2. Validates that `authorize_url` and `token_url` are reachable
3. Returns `{"status": "ok", "discovery": {...}}` or `{"status": "error", "error": "..."}`

## Tests

- `tests/integration/provider_admin/mod.rs` -- **10 integration tests**
  - List providers (empty and populated)
  - Create provider (valid, validation errors)
  - Get provider by id
  - Update provider config
  - Delete provider
  - Set secret / secret redacted in GET
  - Test provider connection
  - Discovery document includes providers list

## Related decisions

- `decisions/00-identity-broker-pattern.md` -- provider admin model

## Implementation notes

- **Admin endpoints are unauthenticated in this feature.** Authorization (admin token, Cedar policy gate) is a follow-up step.
- All admin endpoints operate through `ProviderService` -- no direct DB access in handlers
- Provider secrets are encrypted at rest; the secret endpoint encrypts before storage via `ProviderCrypto`
- The discovery extension is additive -- existing OIDC discovery fields are unchanged
- Tests use real Turso/LibSQL database behind `ProviderService` -- integration tests, not mocks
