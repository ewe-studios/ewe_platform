# Spec 86 Progress: Auth Primitives

_Last updated: 2026-11-22._

**Abbreviations.** Continuous integration (CI); Application programming
interface (API); mutual Transport Layer Security (mTLS); Rust package (crate).

## Status at a glance

| Task | State |
|---|---|
| T1 — `Secret`, the `ConfidentialText` alias, redacted output, `ct_eq` | ✅ Done |
| T2 — One `constant_time_eq`; delete the copies | ✅ Done |
| T3 — `extract_bearer_token` over `Secret::from_bearer`; `bearer_header` | ✅ Done |
| T4 — `AuthInfo` moves to `foundation_auth`; connectrpc re-exports it | ✅ Done |
| T5 — `CallCredentials`, `ClientAuth`, `AuthError`, `Principal`, `AuthMethod` | ✅ Done |
| T6 — `StaticToken`; version bumps and consumers | ✅ Done |

## What landed

### T2 — one constant-time comparison

- `foundation_auth/src/shared/mod.rs` holds `pub fn constant_time_eq`.
- `shared/two_factor.rs` re-exports it, so the existing path stays.
- `server/models/client.rs`, `server/models/token.rs` and
  `shared/password_hash.rs` call the shared function.
- `foundation_keychain/src/core/crypto.rs` re-exports the shared function. It
  was a fifth copy that the specification did not list.
- `tests/auth_primitives_tests.rs` holds `one_constant_time_eq`, a source scan.

### T1 — `Secret`

- `server_credential/secret.rs` holds `Secret`, `expose`, `is_empty`, `ct_eq`,
  `from_bearer`, redacted `Debug` and `Display`, constant-time `PartialEq`, and
  serde. `ConfidentialText` is an alias for one release.
- `shared/types.rs` re-exports the alias.
- Every old `ConfidentialText::get()` call site uses `expose()`.

### T3 — bearer parser

- `server_credential/bearer.rs` holds `extract_bearer_token` and
  `bearer_header`.
- `shared/middleware.rs::extract_bearer_token` keeps its path and delegates to
  the new parser.

### T4 — `AuthInfo` has one home

- `server_credential/client_auth.rs` holds `AuthInfo`.
- `foundation_connectrpc/src/shared/auth.rs` re-exports
  `foundation_auth::AuthInfo`. `AuthFunc` and the interceptors do not change.
- `foundation_connectrpc/tests/auth_tests.rs` holds `auth_info_single_home`.

### T5 — the client-auth trait

- `server_credential/call.rs` holds `CallCredentials`.
- `server_credential/client_auth.rs` holds `AuthInfo`, `Principal`,
  `AuthMethod`, `AuthFuture`, `ClientAuth` and `AuthError`.
- `AuthMethod` has the `SharedToken` variant.

### T6 — `StaticToken` and versions

- `server_credential/static_token.rs` holds the `StaticToken` provider.
- `foundation_auth` moves from `0.0.8` to `0.0.9`.
- `foundation_ai`, `foundation_keychain`, `foundation_connectrpc` and
  `examples/cf-login-app` name the new requirement.

## The test file

All `foundation_auth` tests are in
`backends/foundation_auth/tests/auth_primitives_tests.rs`:

`one_constant_time_eq`, `secret_is_redacted`, `secret_eq`,
`secret_serde_round_trip`, `bearer_case_insensitive`, `bearer_control_byte`,
`bearer_no_space`, `bearer_round_trip`, `static_token_no_header_is_none`,
`static_token_missing`, `static_token_accepts_match`,
`static_token_constant_time`, `auth_error_no_secret`, `auth_info_artifacts` and
`client_auth_is_object_safe`.

`auth_info_single_home` is in
`backends/foundation_connectrpc/tests/auth_tests.rs`, because only a connectrpc
test can name both `AuthInfo` paths.

## Verify (2026-11-22, macOS, profile `uat`)

| Command | Result |
|---|---|
| `cargo test -p foundation_auth --profile uat` | Pass. 65 tests. |
| `cargo test -p foundation_connectrpc --profile uat auth` | Pass. `auth_tests`: 15 tests. |
| `cargo clippy -p foundation_auth -p foundation_connectrpc --profile uat -- -D warnings` | Pass, no warnings. The command also checks the path dependencies; none warned. |
| `cargo check -p foundation_ai --profile uat --no-default-features` | Pass. The four always-compiled providers compile. |
| `cargo check -p foundation_keychain --profile uat` | Pass. |
| `cargo fmt --all -- --check` | Pass. |
| `grep -rn "fn constant_time_eq" backends/` | One hit: `foundation_auth/src/shared/mod.rs`. |
| `grep -rn "struct Secret" backends/` | 18 hits. One is the credential type in `foundation_auth/src/server_credential/secret.rs`. See issue #674. |

The `foundation_ai` check leaves `llamacpp` and `candle` off. The two
one-line changes in `huggingface_gguf_provider` and
`huggingface_candle_provider` need those heavy features, so this check does
not compile them. Both use the same `SecretOnly(t) => t.expose().to_owned()`
form as the compiled code.

## Commits (branch `spec/86-auth-primitives`)

| Task | Commit |
|---|---|
| T2 — one `constant_time_eq` | `f0bc921ab` |
| T1 — `Secret` | `cd6c17dfb` |
| T3 — bearer parser | `d8d872097` |
| T4 — `AuthInfo` move | `4ef299ebc` |
| T5 — `ClientAuth` trait | `a2b94660b` |
| T6 — `StaticToken` and versions | `23e932596` |

## Known issues

| Finding | Severity | State |
|---|---|---|
| Four private copies of `constant_time_eq` in `foundation_auth`, and a fifth in `foundation_keychain` | low | [#554](https://github.com/ewe-studios/ewe_platform/issues/554); closed by T2 |
| `ConfidentialText::Display` shows the first character and the length | low | [#555](https://github.com/ewe-studios/ewe_platform/issues/555); closed by T1 |
| The section 8 `struct Secret` grep matches generated and test types | low | [#674](https://github.com/ewe-studios/ewe_platform/issues/674); open |

## Next steps

1. Merge the branch after the owner reviews it.
2. #554 and #555 are closed in this change.
3. #674 is a low-severity documentation defect. It does not block this work.

## Open questions

None. Section 10 of `requirements.md` records the five decisions.
