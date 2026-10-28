---
feature: "foundation_auth: proof-of-work check, constant-time tags, register status, header compare, integer casts, OpenSSL, wasm deps, duplicate stores, streamed bodies, async-only OAuth"
status: "pending"
priority: "critical"
depends_on: ["03-client-body-readiness"]
branch: "fix/io-18-auth"
crates: ["foundation_auth", "foundation_db"]
backlog: "docs/bug-backlog.md §12 (10 entries); TokenSigner non-constant-time compare (recorded on PR #157's branch)"
---

# Feature 18: `foundation_auth`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** proof of work (PoW); hash-based message authentication code
(HMAC); identity provider (IdP); Open Authorization (OAuth); OpenSSL version 3 (OpenSSL 3);
WebAssembly (wasm); Hypertext Transfer Protocol (HTTP); pull request (PR); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Security fixes first (E1, E2), each its own commit with its test. Use the shared protocol in
[`requirements.md`](../../requirements.md) sections 3 and 4.

## T0. Check that every entry is still open (table in the PR body)

## E1. `PowService` checks leading zero bits against a byte count (security)

- **Where:** `src/server/services/pow_service.rs` `verify_solution` compares
  `count_leading_zero_bytes(&hash)` (it counts bits) with `difficulty / 8`: difficulty 22 needs
  only 2 zero bits. `solve_validity_secs` is never read.
- **Fix:** compare bits with `difficulty`; rename the helper `count_leading_zero_bits`; enforce
  the solve window (reject a solution older than `solve_validity_secs`), or drop the field —
  enforce it unless the spec for this service says otherwise.
- **Tests:** `pow_rejects_solution_below_difficulty` (a hash with 21 zero bits at difficulty 22);
  `pow_accepts_exact_difficulty`; `pow_rejects_expired_solution`. Update tests that solved at
  difficulty 8.

## E2. `TokenSigner::verify` compares HMAC tags in non-constant time (security)

- **Where:** `src/shared/session.rs:384`:
  `expected.iter().zip(&actual).all(|(a, b)| a == b)`.
- **Fix:** `Mac::verify_slice` from the `hmac` crate (constant time). Also check the lengths
  first through `verify_slice` (it does). Leave key ids to spec 69's signer, but add a backlog
  line that `TokenSigner` has one key and no key id.
- **Test:** `token_signer_rejects_modified_tag` and `token_signer_rejects_truncated_tag`
  (the old code accepts a truncated tag: `zip` stops at the shorter one — check and say).

## E3. `register` answers a duplicate email with `200 {"error":"user_exists"}`

- **Fix:** return `IdpError::Conflict` (409), per spec 48 F03. **Test:** `register_duplicate_email_is_409`.

## E4. `header_name_eq` compares names through `Debug` formatting

- **Fix:** compare canonical header names (case-insensitive). **Test:** `header_name_eq_ignores_case_and_debug_format`.

## E5. IdP integer columns are cast with `as`

- **Where:** `interval_secs`, `counter`, `failed_login_attempts` in `src/server/storage.rs`.
- **Fix:** `u32::try_from` (and the right width per column); a `Parse` error on overflow or
  negative. **Test:** `negative_counter_is_a_parse_error`.

## E6. `server-native` tests do not link (OpenSSL 3 symbols)

- **Fix:** enable `openssl/vendored` for that feature (preferred: hermetic), or document the system
  dependency and install it in the CI image. Say which.
- **Verify:** `cargo test -p foundation_auth --features server-native --no-run` links here.

## E7. Unused dependencies on wasm

- **Fix:** make `argon2`, `foundation_errstacks`, `pbkdf2`, `time`, `uuid` target-specific or
  feature-gated as used; gate `constant_time_eq` with its native caller.
- **Verify:** `cargo check --target wasm32-unknown-unknown -p foundation_auth --no-default-features --features wasm 2>&1 | rg 'unused|never used'` is empty.

## E8. `WasmCredentialStore` and `session` duplicate `CredentialStorage`

- **Fix:** move callers onto `CredentialStorage` over a D1 `StorageProvider`; delete
  `foundation_db::wasm::{WasmCredentialStore, session}` (this also removes the inline test module
  in `wasm_credential_store.rs`). Coordinate with feature 17 E10 if both are open.
- **Verify:** wasm builds of `foundation_auth`, `foundation_db`, `examples/cf-login-app`.

## E9. `NativeOAuth` and `PasswordAuthClient` read an empty body

- **Where:** `src/native/oauth.rs` (`extract_body_text`, `error_body_text`),
  `src/native/password_auth.rs` (`extract_body`) read through `get_body_ref()`; the client gives a
  lazy `SendSafeBody::Stream`, read as empty.
- **Fix:** take the body and read it as a stream (status first; error body up to a configured
  limit, 64 KiB as `foundation_deployment`'s `BodyLimits`).
- **Tests:** `native_oauth_parses_token_response` and `password_auth_reports_error_message`
  against a local server (they fail today: empty body).

## E10. `shared` HTTP clients read bodies synchronously and whole

- **Where:** `discovery`, `jwks`, `introspection`, `userinfo`, `upstream_client` (`src/shared/`)
  call sync `try_collect_bytes` inside `async fn`s; error bodies read whole before the status is
  used.
- **Fix:** `collect_bytes_async` (`AsyncSendSafeBody`), status first, error body capped (config).
- **Tests:** one per client: success parses; error body over the cap is cut and reported; a
  ticker keeps ticking while the server delays 300 ms.

## E11. `NativeOAuth` keeps sync twins

- **Fix:** keep only the async methods (`exchange_code`, `client_credentials`, `refresh_token`
  as `async fn`, without `_async`); callers `.block()` at real boundaries.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_auth --profile dev    # includes wasm
python3 scripts/ci/ci.py run foundation_db --profile dev
cargo test -p foundation_auth --features server-native --profile dev
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] T0 table; every open entry fixed; backlog entries deleted.
- [ ] E1 and E2 tests fail on the base code and pass now (shown in the PR body).
- [ ] `server-native` tests link and pass. CI green (native and wasm).

## Review checklist (main session)

- [ ] E1: the difficulty unit is bits everywhere (client solver too).
- [ ] E2: no other tag or token comparison in the crate uses `==` on secret bytes
      (`rg -n '== *expected|expected *==' src`).
