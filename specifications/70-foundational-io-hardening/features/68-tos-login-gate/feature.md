---
feature: "foundation_auth: the ToS gate at login, with a bound single-use code; tos/accept is never trusted with a user id from the body"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_auth"]
catalogue_rows: [57, 58]
---

# Feature 68: ToS gate at login

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

**Owner decision (2026-11-02):** build the ToS-code flow properly (feature 66 deleted the
stub because nothing used it; the flow itself is part of the design and was never built).
Runs next in lane B (security).

## Design source

Spec 48 feature 02 (login step 7) and feature 07: during login, if the active ToS version is
newer than the user's last acceptance, the server answers
`206 {"status": "tos_required", "tos_await_code": "<code>", "tos_version": "<v>"}` instead of
creating the session. The client shows `GET /auth/v1/tos/latest` and posts the acceptance.

## Problems (verified on the lane B branch)

1. `login` (`src/server/handlers/core.rs` ~648) never checks ToS: no gate.
2. **Security:** `tos_accept` (~1463) reads `user_id` from the JSON body with no
   authentication and records acceptance for that user. Anyone can accept ToS for any user.

## Design

1. **Gate.** In `login`, after the password (and after MFA succeeds, on the MFA-completion path
   and on every other path that ends in "authenticated": passkey, social, device code — find them
   all with `rg -n '"authenticated"'`), check ToS: if an active version exists and the user has not
   accepted it, do not create the session; issue a ToS code and answer 206 as above. If no ToS
   version is configured, there is no gate.
2. **Code.** 256 random bits, base64url. Store only `SHA-256(code)` in the service KV, with
   `{user_id, tos_version, pending_login}` where `pending_login` holds what is needed to finish
   the login (client id, redirect URI, the auth method that succeeded). TTL `tos_code_ttl` on
   the IdP config (default 10 min, like an authorization code — say so). Single use (delete on
   use, before acting). Compare by looking up the hash (no secret comparison by `==`).
3. **Accept.** `POST /auth/v1/tos/accept` accepts **either**:
   - `{"tos_await_code": "...", "tos_version": "..."}`: look up the code; the version must match
     the code's version and still be the active one; record acceptance for the code's user; then
     finish the pending login (create the session, same response as a normal successful login:
     202 + Location + cookies); or
   - an authenticated session (session cookie or bearer, as the other authenticated handlers
     read it) with `{"tos_version": "..."}`: record acceptance for the **session's** user.
   - A `user_id` in the body is ignored (and documented as ignored). Neither code nor session → 401.
4. Errors: unknown, used or expired code → 401 `invalid_tos_code` (one message for all three);
   wrong or stale version → 409 `tos_version_mismatch` with the active version.
5. Keep `TosService` the owner of versions, acceptances and codes. No pretend checks anywhere
   (`rg -n 'In production|In a real implementation' backends/foundation_auth/src` stays empty).

## Tests (`backends/foundation_auth/tests/`; the first three fail on the current code)

- `login_with_unaccepted_tos_returns_206_and_no_session`
- `tos_accept_ignores_user_id_in_body` (an unauthenticated accept with a victim's `user_id`
  → 401, and the victim's acceptance is not recorded)
- `tos_code_completes_the_login` (206 → accept with the code → 202 + session cookie)
- `tos_code_is_single_use`, `tos_code_expires`, `tos_code_is_bound_to_its_user` (a code issued
  for user A cannot record acceptance for user B), `stale_tos_version_is_409`
- `authenticated_user_can_accept_new_tos_version`
- `no_active_tos_means_no_gate`
- the MFA path: `mfa_then_unaccepted_tos_returns_206`

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_auth --profile dev
```
Flake check: 20 runs of each new test.

## Done when

- [ ] Every path to "authenticated" passes the ToS gate; `tos/accept` never trusts a body
      `user_id`; codes are hashed, bound, expiring and single-use.
- [ ] The first three tests fail before and pass after (show both). CI green.
