---
feature: "UserInfo Client"
description: "Fetch user profile from /oidc/userinfo with bearer token"
status: "completed"
priority: "high"
depends_on: ["03-oidc-discovery"]
estimated_effort: "small"
created: 2026-06-05
last_updated: 2026-06-07
author: "Main Agent"
tasks:
  completed: 1
  uncompleted: 0
  total: 1
  completion_percentage: 100%
---

# Feature 04: UserInfo Client

## Description

Fetch user profile information from the OIDC `/oidc/userinfo` endpoint using a bearer token. Returns standard OIDC claims about the authenticated user.

## Module

`backends/foundation_auth/src/shared/userinfo.rs` — shared types

## API Surface

```rust
/// User profile from OIDC UserInfo endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub sub: String,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub middle_name: Option<String>,
    pub nickname: Option<String>,
    pub preferred_username: Option<String>,
    pub profile: Option<String>,
    pub picture: Option<String>,
    pub website: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub gender: Option<String>,
    pub birthdate: Option<String>,
    pub zoneinfo: Option<String>,
    pub locale: Option<String>,
    pub phone_number: Option<String>,
    pub phone_number_verified: Option<bool>,
    pub address: Option<serde_json::Value>,
    pub updated_at: Option<String>,
    pub groups: Option<Vec<String>>,
}

/// UserInfo client — fetches user profile from OIDC provider.
pub struct UserInfoClient;

impl UserInfoClient {
    /// Fetch user profile using the given access token.
    pub async fn fetch(userinfo_url: &str, access_token: &str) -> Result<UserInfo, UserInfoError>;
}
```

## Implementation Details

### HTTP request
- `GET {userinfo_url}` with header `Authorization: Bearer {access_token}`
- `Accept: application/json`
- Returns JSON body → `serde_json::from_str` → `UserInfo`

### Native implementation
- `SimpleHttpClient::get(url).header("Authorization", format!("Bearer {}", token)).send_async()`

### Wasm implementation
- Browser fetch API with `Authorization` header
- Gated behind `wasm-bindgen-oauth` feature

### Error type
```rust
pub enum UserInfoError {
    InvalidUrl(String),
    FetchFailed(String),
    Unauthorized(String),
    ParseError(String),
    MissingSubject,
}
```

## Dependencies

- Existing: `foundation_netio` (native)
- Existing: `serde`, `serde_json`

## Testing

- Parse full UserInfo JSON → all fields populated
- Parse minimal UserInfo JSON → only `sub` present, optional fields None
- Missing `sub` field → `MissingSubject` error
- Error: 401 response → `Unauthorized`
- Error: invalid JSON → `ParseError`

## Sync/Async Notes

The `async fn` methods shown are the primary implementation. For sync callers,
use valtron bridging: `from_future` + `execute` + `collect_one`. See the valtron skill.
