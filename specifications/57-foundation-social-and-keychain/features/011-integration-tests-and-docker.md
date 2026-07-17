# F03: Integration Tests + Docker Packaging

## Goal

Shared integration test suite that runs against both backends, plus Docker packaging for the native server.

## Work

### 1. Integration Tests (`tests/integration/`)

All tests run against **both backends** (feature-gated test targets):

**Auth flow** (`auth.test.rs`):
- `POST /accounts/prelogin` → returns KDF params for known/unknown user
- `POST /identity/accounts/register` → creates user, stores password hash
- `POST /identity/connect/token` (password grant) → returns `LoginResponse` with JWT access + refresh tokens
- `POST /identity/connect/token` (refresh grant) → rotates tokens
- `GET /api/accounts/profile` → returns `ProfileResponse`
- `PUT /api/accounts/profile` → updates name
- `POST /api/accounts/verify-password` → verifies master password
- `POST /api/accounts/password` → changes password (rotates security stamp, kills devices)
- `DELETE /api/accounts` → deletes user + cascades all data

**Vault CRUD** (`vault.test.rs`):
- `GET /api/sync` → returns full sync: profile + ciphers + folders + collections + policies + sends + domains
- `POST /api/ciphers` → creates cipher (login, note, card, identity types)
- `GET /api/ciphers` → lists all ciphers
- `PUT /api/ciphers/:id` → updates cipher
- `DELETE /api/ciphers/:id` → hard deletes
- `PUT /api/ciphers/:id/delete` → soft deletes (sets `deleted_at`)
- `PUT /api/ciphers/:id/restore` → restores from trash
- `POST /api/ciphers/:id/attachment/v2` → initializes attachment upload
- `POST /api/ciphers/:id/attachment/:att_id` → uploads attachment bytes
- `POST /api/folders` → creates folder
- `PUT /api/folders/:id` → updates folder
- `DELETE /api/folders/:id` → deletes folder

**Sends** (`sends.test.rs`):
- `POST /api/sends` → creates text send
- `POST /api/sends/file/v2` → creates file send, returns upload URL
- `POST /api/sends/:send_id/file/:file_id` → uploads file bytes
- `POST /api/sends/access/:access_id` → anonymous access (with password)
- `GET /api/sends/:send_id/:file_id?t=<jwt>` → downloads file via JWT-gated URL
- `DELETE /api/sends/:id` → deletes send + blob

**Organizations** (`organizations.test.rs`):
- `POST /api/organizations` → creates org, auto-creates owner membership + collection
- `POST /api/organizations/:org_id/users/invite` → invites member
- `POST /api/organizations/:org_id/users/:member_id/accept` → accepts invite
- `POST /api/organizations/:org_id/users/:member_id/confirm` → confirms member
- `POST /api/organizations/:org_id/collections` → creates collection
- `PUT /api/organizations/:org_id/collections/:col_id/users` → sets collection users
- `PUT /api/ciphers/:id/share` → shares cipher to org
- `GET /api/organizations/:org_id/collections/details` → returns collection access details

**2FA** (`two_factor.test.rs`):
- `POST /api/two-factor/get-authenticator` → returns TOTP secret
- `POST /api/two-factor/authenticator` → enables TOTP (verifies code)
- `POST /api/two-factor/get-recover` → returns recovery code
- Login with TOTP → requires 2FA step
- Login with recovery code → disables 2FA

**Cron** (`cron.test.rs`):
- Expired sends get purged (deleted from DB + blob storage)
- Trashed ciphers older than 30 days get purged

### 2. Crypto Compatibility Tests

Verify that both backends produce identical output:
- PBKDF2: same `(password, salt, iterations)` → same derived key
- HMAC-SHA256: same `(key, data)` → same MAC
- JWT: sign on one backend → verify on the other
- TOTP: same secret + time step → same code

### 3. Docker Packaging

**Multi-stage Dockerfile**:
```dockerfile
FROM rust:1.80 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY backends/foundation_db/ ./backends/foundation_db/
COPY backends/foundation_auth/ ./backends/foundation_auth/
COPY backends/foundation_http/ ./backends/foundation_http/
COPY backends/foundation_netio/ ./backends/foundation_netio/
COPY backends/foundation_keychain/ ./backends/foundation_keychain/
RUN cargo build --release --features backend-native -p foundation_keychain

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/foundation_keychain /usr/local/bin/
COPY --from=builder /app/backends/foundation_keychain/migrations/ /app/migrations/
EXPOSE 8080
ENV KEYCHAIN_DATA=/data
ENV LISTEN_ADDR=0.0.0.0:8080
VOLUME /data
ENTRYPOINT ["foundation_keychain", "serve"]
```

**`docker-compose.yml`**:
```yaml
services:
  keychain:
    build: .
    ports: ["8080:8080"]
    volumes: ["keychain-data:/data"]
    environment:
      - KEYCHAIN_DATA=/data
      - LISTEN_ADDR=0.0.0.0:8080
volumes:
  keychain-data:
```

### 4. Bitwarden CLI End-to-End Test

```bash
bw config server http://localhost:8080
bw register --email test@test.com --password test1234
bw login test@test.com test1234
bw sync
bw create item '{"type":1,"name":"test","login":{"username":"u","password":"p"}}'
bw list items
bw delete item <id>
```

### Success Criterion

- All integration tests pass against both backends
- Crypto compatibility tests pass (bit-for-bit matching)
- Docker image builds and runs
- `docker compose up` → Bitwarden browser extension connects successfully
- Bitwarden CLI end-to-end test passes (register → login → sync → create → delete)
