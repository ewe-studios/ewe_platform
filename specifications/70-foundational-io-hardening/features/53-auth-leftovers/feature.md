---
feature: "foundation_auth leftovers: dead wasm session file, async-only OAuth names, TokenSigner key ids, server-tree clippy in CI"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_auth", "foundation_db"]
catalogue_rows: [19]
---

# Feature 53: auth leftovers

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## E1. Dead `foundation_db/src/wasm/session.rs`

`wasm/mod.rs` has `// pub mod session;`; the file is never compiled (spec 70 feature 18 E8
said delete it). Delete the file and the commented line.

## E2. OAuth methods keep the `_async` suffix (spec 70 feature 18 E11)

`NativeOAuth::{exchange_code_async, client_credentials_async, refresh_token_async}`
(`src/native/oauth.rs`). The sync twins are gone; rename to `exchange_code`,
`client_credentials`, `refresh_token` and update every caller
(`rg -n '_async\(' --glob '*.rs' backends examples apps bin` for these names) and docs.

## E3. `TokenSigner` has one key and no key id

`src/shared/session.rs`: a signed token is `token.sig`; there is no key id, so the key
cannot rotate without logging out every session. Implement rotation:
`TokenSigner::new(keys: KeyRing)` where the ring has one **current** key (signs) and zero or
more **previous** keys (verify only), each with a short id; tokens become
`token.kid.sig`; verification picks the key by id (constant-time tag compare stays,
`Mac::verify_slice`); a token without a kid is verified with the current key during a
migration window (config flag, default on, documented) so existing sessions survive.
Tests: `rotated_key_still_verifies_old_tokens`, `unknown_kid_is_rejected`,
`legacy_token_without_kid_verifies_when_allowed` and `…_rejected_when_disabled`.

## E4. Server-tree clippy debt (backlog §12, 212 lints) and inline tests

- Fix all clippy warnings under `--features server-native` and `server-test`. Mechanical
  only.
- Add the clippy run for `server-test` to `ci/packages.toml` so it stays at zero.
- Move the inline tests in `src/shared/two_factor.rs` and `src/shared/auth_session.rs` to
  `tests/` (rule 7).

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_auth --profile dev
cargo clippy -p foundation_auth --profile dev --all-targets --features server-native 2>&1 | rg -c '^warning'  # 0
rg -l '#\[cfg\(test\)\]' backends/foundation_auth/src   # no output
```

## Done when

- [ ] E1–E4 done; §12 entry deleted; key-rotation tests pass. CI green (native and wasm).
