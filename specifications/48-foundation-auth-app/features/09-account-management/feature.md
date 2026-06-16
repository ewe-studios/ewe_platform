---
feature: "Account Management"
description: "User info CRUD, session listing, passkey management, account deletion"
status: "pending"
priority: "medium"
depends_on: ["02-login-mfa-handlers"]
estimated_effort: "medium"
created: 2026-06-16
---

# Feature 09: Account Management

## Description

Account page API endpoints. Powers the AccountPage UI component.

## Endpoints

### GET /auth/v1/users/{id}

Returns user info (email, username, created_at, etc.).

### PUT /auth/v1/users/{id}

Update user info (preferred_username, given_name, family_name).

### POST /auth/v1/users/{id}/change_password

```json
{
  "current_password": "old-password",
  "new_password": "new-password"
}
```

### GET /auth/v1/users/{id}/sessions

List active sessions with IP, user_agent, created_at, is_current.

### DELETE /auth/v1/users/{id}/sessions/{session_id}

Revoke a specific session.

### DELETE /auth/v1/users/{id}/webid/{passkey_id}

Remove a passkey (delegates to WebAuthnService).

### POST /auth/v1/users/{id}/revoke

Delete account and all associated data.

## UserService extensions

```rust
pub async fn update_user(
    &self,
    user_id: &str,
    updates: &UserUpdateRequest,
) -> Result<(), UserServiceError>;

pub async fn change_password(
    &self,
    user_id: &str,
    current_hash: &str,
    new_password: &str,
) -> Result<(), UserServiceError>;

pub async fn delete_account(&self, user_id: &str) -> Result<(), UserServiceError>;
```

## Module changes

- `backends/foundation_auth/src/server/handlers/core.rs` — add account endpoints
- `backends/foundation_auth/src/server/services/user_service.rs` — extend
- `backends/foundation_auth/src/server/idp_server.rs` — register routes

## Testing

- User info fetched correctly
- User info updated
- Password changed with current password verification
- Sessions listed
- Session revoked
- Account deletion removes all data
