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

## In progress

- **E9 — `NativeOAuth` / `PasswordAuthClient` empty body.** These two native
  clients still read the response through `get_body_ref()`, which is empty for a
  lazy `SendSafeBody::Stream`.
- **E10 — `shared` clients sync drain.** The five `shared` clients
  (`discovery`, `jwks`, `introspection`, `userinfo`, `upstream_client`) still
  drain the body with the sync `try_collect_bytes` inside `async fn`, and read an
  error body whole before the status.

## Next

1. Fix E9 (streamed body read, status first, bounded error body).
2. Fix E10 (async drain, status first, bounded error body).
3. Add the tests named in each entry.
4. Update this file and `learnings.md`.
