---
feature: "foundation_auth security follow-ups: atomic single-use take, cache TTL, TOTP enrollment/encryption/replay, social open redirect, logout revocation, failed-attempt reset"
status: "done"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_auth", "foundation_db"]
catalogue_rows: [71, 72, 73, 74, 75, 76]
---

# Feature 73: auth security follow-ups

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 68 (`docs/bug-backlog.md` foundation_auth section). Runs next in lane B
(security first). One commit per E; a regression test first for each.

## E1. No atomic single-use take (security)

ToS codes and MFA challenges are read, then deleted, in two steps. Two requests with the same
code at the same time can both pass. Add an atomic take to the store the cache uses
(`KeyValueStore::take(key) -> Option<V>` in foundation_db; SQL backends:
`DELETE … RETURNING`; memory: one lock; KV/D1/R2: say what each can guarantee and document
the non-atomic ones), and use it in `TosService::take_code` and the MFA challenge consume.
Test: `concurrent_use_of_one_tos_code_lets_exactly_one_through` (50 concurrent accepts, 1
success) and the same for an MFA challenge.

## E2. The cache has no TTL

Expired codes and challenges stay in the store. Add an expiry to cache entries (a column or
an expiry field read on get, plus a cleanup in the existing `AsyncStorageCleanup` path), so a
read after expiry returns none and cleanup deletes them. Test:
`expired_entries_are_not_returned_and_are_cleaned`.

## E3. TOTP: no enrollment, secret stored in plain text, codes can be replayed (security)

- Enrollment endpoints (authenticated): start (create secret, return the provisioning URI) and
  confirm (verify a first code, then store). Never overwrite an enrolled secret without the
  current code.
- Encrypt the stored secret at rest with a key from the IdP config (AEAD, for example
  ChaCha20-Poly1305 or AES-GCM with a random nonce; the key from config, never in code). Key
  rotation: store a key id with the ciphertext.
- Replay: remember the last accepted time step per user and reject a code for the same or an
  earlier step.
Tests: `enroll_then_login_with_totp`, `totp_secret_is_not_stored_in_plain_text`,
`totp_code_cannot_be_used_twice`, `enrollment_requires_session`.

## E4. Social login open redirect (security)

`social_authorize` accepts any `redirect_uri`, and the authorization code is sent to it. Check
it against the client's registered redirect URIs (exact match, as `login` does after feature
68). Test: `social_authorize_rejects_unregistered_redirect_uri`.

## E5. `logout` does not revoke the session

Read the session cookie (or bearer) and revoke that session; clear the cookie. Test:
`logout_revokes_the_session` (the old cookie no longer works).

## E6. `login` does not reset failed attempts after success

Reset `failed_login_attempts` on success (password and MFA paths). Test:
`successful_login_resets_failed_attempts`.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_auth --profile dev
python3 scripts/ci/ci.py run foundation_db --profile dev
```
Clippy 0 for server-test and server-native. Flake check: 20 runs of each new test.

## Done when

- [x] Six entries fixed with tests that fail before and pass after. Entries deleted. CI green.
