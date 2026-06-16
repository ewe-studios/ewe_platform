---
feature: "Password Reset"
description: "Password reset request (magic link) and set via magic link token"
status: "pending"
priority: "high"
depends_on: ["05-proof-of-work"]
estimated_effort: "small"
created: 2026-06-16
---

# Feature 04: Password Reset

## Description

Two-endpoint password reset flow:
1. User requests reset → server creates magic link token
2. User clicks magic link → sets new password

## Endpoints

### POST /auth/v1/users/request_reset

Request:
```json
{
  "email": "user@example.com",
  "pow": "nonce-solution"
}
```

Response: `202 {"status": "reset_requested"}` (always returns success — prevents email enumeration)

Creates a `PasswordReset` record with a random token and expiry.

### PUT /auth/v1/users/{id}/reset

Request:
```json
{
  "new_password": "Str0ng!NewPass#2026",
  "magic_link_id": "token-from-email"
}
```

Response: `200 {"status": "password_updated"}`

Error:
- `400 {"error": "invalid_token"}` — token not found or expired
- `400 {"error": "password_policy_violation", "details": [...]}` — new password too weak

## Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordReset {
    pub id: String,
    pub user_id: String,
    pub token: String,  // random token for magic link
    pub expires_at: i64,
    pub created_at: i64,
}

impl PasswordReset {
    pub fn new(user_id: String, ttl: Duration) -> Self {
        let now = Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            token: generate_random_token(),
            expires_at: now + ttl.as_millis() as i64,
            created_at: now,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp_millis() >= self.expires_at
    }
}
```

## Handler

```rust
impl IdpHandlerCore {
    pub async fn request_reset(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<serde_json::Value, IdpError> {
        // 1. Parse JSON body
        // 2. Validate PoW
        // 3. Look up user by email (if not found, still return success — no enumeration)
        // 4. Create PasswordReset record, persist
        // 5. Return 202 {"status": "reset_requested"}
        //    (email sending is deferred — separate spec)
    }

    pub async fn reset_password(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<serde_json::Value, IdpError> {
        // 1. Parse JSON body: {new_password, magic_link_id}
        // 2. Validate password against policy
        // 3. Look up PasswordReset by token
        // 4. Check not expired
        // 5. Hash new password, update user
        // 6. Delete PasswordReset record
        // 7. Return 200 {"status": "password_updated"}
    }
}
```

## UserService extensions

```rust
pub async fn reset_password(
    &self,
    user_id: &str,
    new_password_hash: &str,
) -> Result<(), UserServiceError>;
```

## Module changes

- `backends/foundation_auth/src/server/models/code.rs` — add PasswordReset model (alongside AuthCode + DeviceCode)
- `backends/foundation_auth/src/server/handlers/core.rs` — add request_reset(), reset_password()
- `backends/foundation_auth/src/server/services/user_service.rs` — add reset_password()
- `backends/foundation_auth/src/server/idp_server.rs` — register routes

## Testing

- Request reset → always returns 202
- Reset with valid token → password updated
- Reset with expired token → 400 invalid_token
- Reset with weak password → 400 policy violations
