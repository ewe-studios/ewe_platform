---
feature: "User Registration"
description: "User registration endpoint with PoW validation, dev mode registration"
status: "pending"
priority: "high"
depends_on: ["05-proof-of-work"]
estimated_effort: "small"
created: 2026-06-16
---

# Feature 03: User Registration

## Description

New endpoint for user registration. Uses existing `UserService::hash_password`
(Argon2id) and `validate_password` (policy check).

## Endpoint

### POST /auth/v1/users/register

Request:
```json
{
  "email": "user@example.com",
  "password": "Str0ng!Pass#2026",
  "preferred_username": "user123",
  "given_name": "John",
  "family_name": "Doe",
  "user_values": {
    "birthdate": "1990-01-01",
    "tz": "America/New_York",
    "phone": "+1234567890"
  },
  "redirect_uri": "https://app.example.com/callback",
  "pow": "nonce-solution"
}
```

Response (success):
```json
{
  "id": "user_uuid",
  "email": "user@example.com",
  "status": "created"
}
```

Error responses:
- `409 {"error": "user_exists"}` — email already registered
- `400 {"error": "password_policy_violation", "details": [...]}` — password too weak
- `400 {"error": "invalid_pow"}` — PoW validation failed

### POST /auth/v1/dev/register

Simplified registration without PoW. Only available when `IS_DEV` is true.

## Handler

```rust
impl IdpHandlerCore {
    pub async fn register(
        &self,
        bag: &ContextBag,
        req: &Request,
    ) -> Result<serde_json::Value, IdpError> {
        // 1. Parse JSON body
        // 2. Validate PoW (unless dev mode)
        // 3. Validate password against PasswordPolicy
        // 4. Check email not already taken (QueryStore)
        // 5. Hash password via hash_password() (Argon2id)
        // 6. Create User entity, persist via QueryStore
        // 7. Return user info
    }
}
```

## UserService extensions

```rust
// user_service.rs — add:
pub async fn create_user(
    &self,
    email: &str,
    password_hash: &str,
    username: Option<String>,
) -> Result<User, UserServiceError>;

pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, UserServiceError>;
```

## Module changes

- `backends/foundation_auth/src/server/handlers/core.rs` — add register()
- `backends/foundation_auth/src/server/services/user_service.rs` — add create_user(), find_by_email()
- `backends/foundation_auth/src/server/idp_server.rs` — register route

## Testing

- Registration creates user with hashed password
- Duplicate email → 409
- Password too weak → 400 with violation details
- Dev mode registration skips PoW
