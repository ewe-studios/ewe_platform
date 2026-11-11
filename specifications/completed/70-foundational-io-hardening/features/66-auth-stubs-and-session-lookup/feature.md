---
feature: "foundation_auth: no security stubs (ToS codes, dev register) and a direct session lookup"
status: "done"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_auth"]
catalogue_rows: [51, 52, 53]
---

# Feature 66: auth stubs and session lookup

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 53 (backlog §12). Runs next in lane B (security first).

## E1. `TosService::verify_tos_code` accepts every code (security)

`src/server/services/tos_service.rs`: `generate_tos_code` returns a random UUID that is not
stored or bound to the user; `verify_tos_code` returns `true` for any code. Today nothing calls
either (`rg -n 'verify_tos_code|generate_tos_code' backends`), but they are public API, so the
first caller gets a check that always passes.

Decision rule: a stub that pretends to check is not allowed (CLAUDE.md "how to work").
- If no flow needs ToS codes (read the ToS acceptance flow and spec 48's ToS feature, if any):
  **delete** both functions; acceptance stays as it is (versions + recorded acceptance).
- If a flow needs them: implement them. The code is random (256 bits, base64url), stored in the
  service's KV as a hash (SHA-256) keyed to `user_id` with an expiry (`tos_code_ttl` on config,
  default 15 min), single use (deleted on success), compared in constant time. Tests:
  `tos_code_is_bound_to_user`, `tos_code_is_single_use`, `tos_code_expires`,
  `random_code_is_rejected`.
Say which, with the evidence.

## E2. `dev_register` has no dev-mode check

`src/server/handlers/core.rs` ~883 (route `POST /auth/v1/dev/register`, `idp_server.rs` ~47)
calls `register` with the comment "check IS_DEV flag". Add `dev_mode: bool` to the IdP server
config (default `false`, documented). When off: do not register the route (and answer 404 if
the handler is reached). When on: keep today's behaviour and log a warning at start. Tests:
`dev_register_is_404_when_dev_mode_is_off`, `dev_register_works_when_dev_mode_is_on`.

## E3. `SessionManager::get_session` scans every session key

`src/shared/session.rs`: `get_cached_session` is a stub that returns `None`; `get_session`
then lists every session key to find the token. Cost grows with the number of sessions, and a
stub is not allowed. Store sessions under a key derived from the token (for example
`session:{sha256(token)}`; never the raw token), look it up directly, and delete the stub. Keep
the "revoke all sessions for a user" path working (an index `user_sessions:{user_id}` if one is
needed). Existing stored sessions: migrate on read or document that existing sessions are logged
out, and say which. Tests: `get_session_is_a_direct_lookup` (a counting store: one `get`, no
`list`), and the existing session tests.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_auth --profile dev
rg -n 'In a real implementation|In production, look up' backends/foundation_auth/src   # no output
```
Flake check: 20 runs of each new test.

## Done when

- [x] No pretend check left; dev register gated; session lookup direct. Three §12 entries
      deleted. CI green.
