---
feature: "Template/Config API"
description: "Template/config API — registration fields, password policy, providers, theme data"
status: "pending"
priority: "medium"
depends_on: []
estimated_effort: "small"
created: 2026-06-16
---

# Feature 08: Template/Config API

## Description

Static configuration endpoints that the UI uses to dynamically configure pages.
No database needed — loaded from `IdpConfig` at startup.

## Endpoints

### GET /auth/v1/templates/config

```json
{
  "is_dev": false,
  "auth_endpoint": "/auth/v1/oidc/authorize",
  "is_reg_open": true,
  "hide_admin_login": true,
  "brute_force_prot": true,
  "passkey_enabled": true,
  "device_flow_enabled": true
}
```

### GET /auth/v1/templates/user_values

```json
{
  "preferred_username": "optional",
  "given_name": "required",
  "family_name": "optional",
  "birthdate": "hidden",
  "tz": "optional",
  "street": "hidden",
  "zip": "hidden",
  "city": "hidden",
  "country": "hidden",
  "phone": "optional"
}
```

### GET /auth/v1/templates/password_policy

```json
{
  "min_length": 12,
  "require_uppercase": true,
  "require_lowercase": true,
  "require_number": true,
  "require_special": true
}
```

### GET /auth/v1/templates/providers

```json
{"providers": []}
```

(Empty until social providers are implemented — separate spec.)

### GET /auth/v1/templates/theme

```json
{
  "client_name": "Default",
  "client_uri": "",
  "logo_url": "",
  "colors": {}
}
```

## Service

```rust
pub struct TemplateService {
    config: Arc<IdpConfig>,
}
```

All endpoints read from config — no DB queries needed.

## Module changes

- `backends/foundation_auth/src/server/services/template_service.rs` — NEW
- `backends/foundation_auth/src/server/handlers/core.rs` — add template endpoints
- `backends/foundation_auth/src/server/idp_server.rs` — register routes

## Testing

- All endpoints return correct values from IdpConfig
- user_values config controls field visibility
- Password policy matches IdpConfig
