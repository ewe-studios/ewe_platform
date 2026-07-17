# LEARNINGS

## Past Discoveries and Mistakes

### 2026-07-17: Spec Creation — Initial Design

**Migrations 006/007 are orphaned client-side tables.** They were created for outbound OAuth in the client library (`shared/oauth.rs`) but never wired up. Do not reuse them for IdP provider config — create new migrations 024/025 with clear `upstream_providers` and `user_provider_links` names.

**Rauthy exploration doesn't cover social login.** The exploration files (`/home/darkvoid/Boxxed/@dev/repo-expolorations/src.auth/src.rauthy/`) cover auth flows, OIDC/OAuth2, JWT, but not social/upstream provider integration. Design decisions were made from first principles (OIDC spec, OAuth2 spec, rauthy's admin UI mentions).

**Identity broker is the right pattern for our use case.** The user wants one central IdP that apps authenticate against, with multiple upstream login options. This means the IdP mints its own JWTs and upstream providers are just authentication sources — not token issuers for downstream apps.

---
