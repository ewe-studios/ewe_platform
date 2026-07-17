---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-auth-social-login"
this_file: "specifications/58-foundation-auth-social-login/start.md"
created: 2026-07-17
---

# Start: foundation_auth Social Login / Upstream IdP Federation

## Agent Workflow

1. Read `requirements.md` (high-level overview + feature index)
2. **Identify language stack** from requirements.md: **Rust**
3. **Read/generate language skills** — `.agents/skills/rust-clean-code/skill.md` → read it
4. Read `LEARNINGS.md` (past discoveries and mistakes — create if not exists)
5. Identify which feature you're working on from the Feature Index
6. Navigate to `features/[feature-name]/start.md` and follow that feature's workflow
7. **ALWAYS UPDATE LEARNINGS.md** after each completed task/milestone

## Key Decisions

Read `decisions/01-identity-broker-pattern.md` before implementing any feature. Key decisions:

| Decision | Summary |
|----------|---------|
| D01 | Identity broker pattern — IdP always mints its own JWTs |
| D02 | Provider selection via `?provider=X` on /authorize, login page fallback |
| D03 | Per-provider callback at `/auth/v1/providers/{id}/callback` |
| D04 | Double-state: independent states, correlated in UpstreamAuthSession (KV store) |
| D05 | Auto-link by verified email, otherwise create new user |
| D06 | Encrypt provider secrets with ChaCha20-Poly1305, Argon2id-derived key |
| D07 | Generic UpstreamOidcClient + ProviderBackend trait for exceptions |
| D08 | Complete IdP's /authorize flow — issue IdP auth code, app exchanges |
| D09 | Redirect to app with error on upstream failure, never hang |
| D10 | New migrations 024 (upstream_providers) + 025 (user_provider_links), leave 006/007 |

## Dependencies

- **spec-38** (foundation-auth-server) — IdP server base
- **spec-57** (foundation-keychain) — optional future secret storage upgrade
- **foundation_db** — QueryStore + KeyValueStore
- **foundation_http** — HTTP server
- **foundation_netio** — HTTP client for upstream calls

---

_Created: 2026-07-17_
