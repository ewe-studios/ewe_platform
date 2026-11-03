---
feature: "foundation_auth correctness follow-ups (register username, sessions, dispatch, social ToS flow) and the unregistered vectors migration"
status: "pending"
lane: "B"
model: "sonnet"
branch: "fix/io-51-60-lane-b"
crates: ["foundation_auth", "foundation_db"]
catalogue_rows: [77, 78, 79, 80, 81, 85, 86, 87, 88, 89, 90]
---

# Feature 74: auth correctness follow-ups

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 68 (`docs/bug-backlog.md` foundation_auth section).

- **E1 Register username.** A second user without `preferred_username` gets 500 (UNIQUE
  `users.username`). Store NULL (or derive a unique username) when none is given; NULL is
  allowed under UNIQUE in SQLite. Test: `two_users_without_username_can_register`.
- **E2 Sessions are in-memory and per route.** The default `SessionService` is in memory, and an
  app built with `register_routes` only gets one per route. Back sessions with the configured
  credential/KV store (the same one the rest of the IdP uses), shared by all routes; keep the
  in-memory store only as an explicit option for tests. Test:
  `session_survives_restart_with_a_persistent_store`, `register_routes_share_one_session_store`.
- **E3 Dispatch by `ends_with`/`contains` in a fixed order.** Replace it with an exact route table
  (method + path → handler) built once. Test: one test per route that the right handler answers,
  and `unknown_path_is_404`.
- **E4 Social ToS on a GET redirect.** The 206 JSON is returned on the browser callback. For the
  social flow, redirect to a configured ToS page with the code in the URL fragment (or a
  documented query parameter), and accept there. Test: `social_callback_with_unaccepted_tos_redirects`.
- **E5 `023_create_vectors.sql` is not registered in `MIGRATIONS`** (foundation_db). Find out
  whether it is meant to run (who uses the `vectors` table: `rg -n vectors backends/foundation_db
  backends/foundation_vectors`). Register it (with a test that it applies) or delete the file and
  say why.

Added from feature 73 (2026-11-03):

- **E6 TOTP uses HMAC-SHA256; most authenticator apps only do SHA-1.** RFC 6238's default and
  what Google Authenticator and most apps implement is SHA-1, 6 digits, 30 s. Generate and verify
  with SHA-1 by default, put the algorithm in the provisioning URI, and keep SHA-256 as a config
  option. Test: a known RFC 6238 SHA-1 test vector verifies; the provisioning URI names the
  algorithm.
- **E7 `enroll/start` can be called again without limit (security).** Each call makes a new
  pending secret, so an attacker with a session can keep trying first codes. Allow one pending
  enrollment per user (replace it, with its own short TTL), and count failed confirms toward the
  account's failed-attempt lockout. Test: `enroll_confirm_attempts_are_limited`.
- **E8 Session cookies are not `Secure` (security).** Set `Secure`, `HttpOnly` and
  `SameSite=Lax` (or `Strict` where the flow allows) by default; a config flag may turn `Secure`
  off only for local http development. Test: the Set-Cookie header has all three.
- **E9 `sweep_expired` is never scheduled.** Run it from the IdP server on an interval from config
  (default 5 min), as a valtron task (not a thread), stopped on shutdown. Test: with a 100 ms
  interval, an expired entry is gone within 1 s.
- **E10 A social-login authorization code has an empty `client_id`.** Carry the client id from
  `social_authorize` through the callback into the code, and check it at the token endpoint. Test:
  `social_code_is_bound_to_its_client`.
- **E11 Workers KV `take` is not atomic.** It cannot be with KV. Make the IdP refuse to start with
  a KV-backed cache for single-use data unless a config flag accepts the risk, and say so in the
  docs; recommend D1 or a Durable Object.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_auth --profile dev
python3 scripts/ci/ci.py run foundation_db --profile dev
```

## Done when

- [ ] E1–E11 fixed (E11 as documented refusal) and their entries deleted. CI green.
