---
feature: "Terms of Service"
description: "ToS versioning, display, and acceptance tracking"
status: "pending"
priority: "medium"
depends_on: ["02-login-mfa-handlers"]
estimated_effort: "small"
created: 2026-06-16
---

# Feature 07: Terms of Service

## Description

ToS versioning and acceptance tracking. During login, if ToS was updated since
the user's last acceptance, return HTTP 206 with `tos_await_code`.

## Endpoints

### GET /auth/v1/tos/latest

Response:
```json
{
  "version": "2.0",
  "content": "# Terms of Service\n\n...",
  "updated_at": "2026-01-01T00:00:00Z"
}
```

### POST /auth/v1/tos/accept

Request:
```json
{
  "user_id": "user_uuid",
  "tos_version": "2.0"
}
```

Response: `200 {"status": "accepted"}`

## Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TosVersion {
    pub id: String,
    pub version: String,
    pub content: String,
    pub created_at: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TosAcceptance {
    pub user_id: String,
    pub tos_version: String,
    pub accepted_at: i64,
}
```

## Migration

```sql
CREATE TABLE tos_versions (
    id TEXT PRIMARY KEY,
    version TEXT NOT NULL UNIQUE,
    content TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_active BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE tos_acceptances (
    user_id TEXT NOT NULL REFERENCES users(id),
    tos_version TEXT NOT NULL,
    accepted_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, tos_version)
);
```

## Login integration

During login (F02 handler):
```rust
// After password verification, before creating session:
if let Some(tos_code) = tos_service.check_on_login(user_id).await? {
    return Ok(json!({
        "status": "tos_required",
        "tos_await_code": tos_code
    }));
}
```

## Module changes

- `backends/foundation_auth/src/server/services/tos_service.rs` — NEW
- `backends/foundation_auth/src/server/handlers/core.rs` — add tos_latest(), tos_accept()
- `backends/foundation_auth/src/server/idp_server.rs` — register routes

## Testing

- ToS fetched correctly
- Acceptance recorded
- Login returns tos_required when ToS updated
