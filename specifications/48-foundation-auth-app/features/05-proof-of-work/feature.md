---
feature: "Proof of Work"
description: "PoW challenge/solve endpoint — anti-bot rate limiting for auth mutations"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-06-16
---

# Feature 05: Proof of Work

## Description

rauthy's anti-bot mechanism. Every auth mutation (login, register, password reset)
requires a valid PoW solution. Not in spec 38 at all.

## How it works

1. Server generates a random challenge + difficulty (leading zero bits required)
2. Client hashes `(challenge + nonce)` until hash has enough leading zeros
3. Server validates by hashing once and checking

Tied to a `browser_id` for per-client tracking.

## Endpoints

### GET /auth/v1/pow — Get challenge params

Response:
```json
{
  "difficulty": 22,
  "challenge": "random-16-byte-hex",
  "expires_in": 30
}
```

### POST /auth/v1/pow — Solve a challenge

Request:
```json
{
  "challenge": "...",
  "solution": "nonce"
}
```

Response: `200 {"valid": true, "expires_in": 300}`

Invalid: `400 {"error": "invalid_pow"}`

## Service

```rust
pub struct PowService {
    challenges: Mutex<HashMap<String, PowChallengeEntry>>,
    difficulty: u32,
    validity_secs: u32,
}

impl PowService {
    pub fn generate_challenge(&self, browser_id: &str) -> PowChallenge;
    pub fn validate(&self, solution: &PowSolution) -> Result<PowResult, PowError>;
    pub fn cleanup(&self);
}
```

Uses existing `sha2` + `rand` (already dependencies).

## PoW validation middleware

```rust
pub fn require_pow(req: &Request, pow_service: &PowService) -> Result<(), IdpError>;
```

Called by login, register, request_reset handlers. Skipped in dev mode.

## Client-side solver (in foundation_auth_ui)

```rust
/// Solve PoW asynchronously with cooperative yielding for wasm.
pub async fn solve_pow_async(challenge: &str, difficulty: u32) -> String;
```

## Module changes

- `backends/foundation_auth/src/server/services/pow_service.rs` — NEW
- `backends/foundation_auth/src/server/handlers/core.rs` — add pow_challenge(), pow_solve()
- `backends/foundation_auth/src/server/idp_server.rs` — register routes

## Testing

- Valid solution passes
- Invalid solution fails
- Expired challenge fails
- Dev mode skips PoW requirement
