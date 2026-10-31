# Feature 18 — foundation_auth — progress

## Landed

- **E1 — proof-of-work (PoW) bit count.** `verify_solution` now compares leading-zero
  **bits** against `difficulty`, not a byte count. The helper is
  `count_leading_zero_bits`. A solution older than `solve_validity_secs` is rejected.
- **E2 — constant-time HMAC tag.** `TokenSigner::verify` uses `hmac`'s
  `verify_slice`, which compares in constant time. The length guard stays.
  Two tests pin a modified tag and a truncated tag.
- **E3 — duplicate email status.** The register handler returns
  `IdpError::Conflict` (HTTP 409) for a duplicate email, not `200`.
- **E4 — header name compare.** `header_name_eq` compares the canonical header
  name (`SimpleHeader`'s `Display`), not its `Debug` form.
- **E5 — integer casts.** `int_to_u32` (`u32::try_from`) replaces three `as u32`
  casts in `storage.rs`. A negative or oversized value is a `Parse` error.
- **E6 — OpenSSL vendored.** `server-native` enables `openssl/vendored`. The
  `server-native` tests link here.
- **E7 — wasm dependencies.** `argon2` and `pbkdf2` are native-only. `time` is
  removed (unused). `uuid` and `foundation_errstacks` are feature-gated.
  `constant_time_eq` in `password_hash.rs` is gated to its callers
  (native argon2/pbkdf2 and `wasm-pbkdf2`).
- **E8 — `WasmCredentialStore` removed.** `wasm_credential_store.rs` and its
  re-export are deleted. The wasm builds of `foundation_auth`,
  `foundation_db`, and `examples/cf-login-app` pass.
- **E9 — streamed body read (native).** `native/oauth.rs` and
  `native/password_auth.rs` take the async body, check the status first, and
  read an error body only up to 64 KiB. They use the shared
  `crate::shared::body` helpers. The cap is wired from
  `OAuthConfig::max_error_body`.
- **E10 — async body read (shared).** The five `shared` clients
  (`discovery`, `jwks`, `introspection`, `userinfo`, `upstream_client`) use
  `collect_bytes_async` (`AsyncSendSafeBody`), check the status first, and cap
  the error body at 64 KiB. The shared helpers live in `src/shared/body.rs`.

## Landed (tests and clippy)

- **E9/E10 tests.** 13 tests in `backends/foundation_auth/tests/integration/body_streaming/`
  (one success parse per client, an over-cap error body is cut, the two native
  cases, and a ticker that keeps ticking while the server delays). The full
  `foundation_auth` suite passes (219 tests).
- **`foundation_auth` clippy pass.** All 107 lib warnings fixed. The last five
  were one root cause: the `expires`/`expires_at` timestamp fields were `f64`.
  They are now `i64` (Unix seconds); the casts are gone and the `#[allow]` on
  `AuthToken::is_expired` is removed. `foundation_connectrpc` and
  `foundation_ai` call sites were updated to the new type.

## Next

1. Run the Linux Docker CI check (`scripts/ci/linux.sh foundation_auth` and
   `foundation_netio`) after the commit.
