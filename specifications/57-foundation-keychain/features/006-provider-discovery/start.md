---
feature: "06-provider-discovery"
spec: "58-foundation-auth-social-login"
depends: "01-provider-model, 02-provider-migrations, 04-social-login-flow"
status: "pending"
---

# Feature 06: Provider Discovery + Admin API

## Goal

Extend the IdP's discovery document to list available upstream providers, and add an admin API for managing providers.

## WHAT

### Extended OIDC Discovery

Add `providers` to the discovery document:

```json
{
  "issuer": "https://auth.example.com",
  "authorization_endpoint": "https://auth.example.com/idp/authorize",
  "...": "...",
  "providers": [
    { "id": "google", "name": "Google", "type": "oidc", "icon_url": "/static/icons/google.svg" },
    { "id": "github", "name": "GitHub", "type": "oauth2", "icon_url": "/static/icons/github.svg" }
  ]
}
```

Apps can use this to dynamically render login buttons: "We support Google and GitHub".

### Admin API Endpoints

All admin endpoints are at `/auth/v1/admin/providers` and require an admin token (future: Cedar policy gate).

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/auth/v1/admin/providers` | List all providers (secrets redacted) |
| `POST` | `/auth/v1/admin/providers` | Create a new provider |
| `GET` | `/auth/v1/admin/providers/{id}` | Get one provider (secret redacted) |
| `PUT` | `/auth/v1/admin/providers/{id}` | Update provider config |
| `DELETE` | `/auth/v1/admin/providers/{id}` | Delete provider |
| `POST` | `/auth/v1/admin/providers/{id}/secret` | Set/update the provider secret |
| `POST` | `/auth/v1/admin/providers/{id}/test` | Test the provider connection (discover + validate config) |

### Create Request Body

```json
{
  "id": "google",
  "name": "Google",
  "provider_type": "oidc",
  "client_id": "12345.apps.googleusercontent.com",
  "discovery_url": "https://accounts.google.com/.well-known/openid-configuration",
  "scopes": ["openid", "email", "profile"],
  "mapping_config": {
    "email_field": "email",
    "email_verified_field": "email_verified",
    "name_field": "name",
    "subject_field": "sub"
  }
}
```

### Secret Endpoint

```json
POST /auth/v1/admin/providers/google/secret
{
  "secret": "GOCSPX-abc123..."
}
```

The secret is encrypted before storage. It is never returned by any endpoint.

### Test Endpoint

Tests the provider configuration without a full login flow:
1. Fetches discovery document (if discovery_url set)
2. Validates that authorize_url and token_url are reachable
3. Returns `{"status": "ok", "discovery": {...}}` or `{"status": "error", "error": "..."}`

## HOW

### Files to modify

- `backends/foundation_auth/src/server/handlers/core.rs` — add `providers_in_discovery`, `admin_list_providers`, `admin_create_provider`, `admin_update_provider`, `admin_delete_provider`, `admin_set_secret`, `admin_test_provider`
- `backends/foundation_auth/src/server/handlers/mod.rs` — re-export
- `backends/foundation_auth/src/server/idp_server.rs` — register admin routes
- `backends/foundation_auth/src/shared/discovery.rs` — add `providers` field

### Admin route registration

```rust
app.route::<ServeAdapter>(SimpleMethod::GET, "/auth/v1/admin/providers");
app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/admin/providers");
app.route::<ServeAdapter>(SimpleMethod::GET, "/auth/v1/admin/providers/{id}");
app.route::<ServeAdapter>(SimpleMethod::PUT, "/auth/v1/admin/providers/{id}");
app.route::<ServeAdapter>(SimpleMethod::DELETE, "/auth/v1/admin/providers/{id}");
app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/admin/providers/{id}/secret");
app.route::<ServeAdapter>(SimpleMethod::POST, "/auth/v1/admin/providers/{id}/test");
```

### Note on Authorization

Admin endpoints are **unauthenticated in this feature**. Authorization (admin token, Cedar policy gate) is a follow-up. In production, gate these endpoints behind the existing middleware system.

---

_Created: 2026-07-17_
