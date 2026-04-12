# Progress - 00b Auth Infrastructure

_Last updated: 2026-04-12_

**Status:** ⬜ Pending — 0 / 30 tasks (0%)

Comprehensive auth infrastructure for `foundation_auth`: JWT, OAuth 2.0
(PKCE S256), credential storage via `foundation_db`, auth state machine,
2FA. Inspired by better-auth's schema and session patterns.

## Blocked On

Nothing — 00a is complete and `foundation_auth` already routes credential
storage through `foundation_db::CredentialStorage`.

## Next Action

Start at `start.md`. The credential-storage bridge is already in place;
this feature builds the JWT, OAuth 2.0 (PKCE S256), auth state machine,
and 2FA layers on top of it.

See [`feature.md`](./feature.md) for the 30-task breakdown, Iron Laws,
and the three-cookie session design.
