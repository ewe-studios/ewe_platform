---
workspace_name: "ewe_platform"
spec_directory: "specifications/81-foundation-auth"
this_file: "specifications/81-foundation-auth/start.md"
created: 2026-11-14
---

# Start: Foundation Auth

## Agent Workflow

1. Read `requirements.md` (purpose, order, what this specification owns and what it does not).
2. Read `../86-auth-primitives/requirements.md` section 3. The trait, `AuthInfo` and `Secret`
   come from there; this specification implements them.
3. Read `../87-mutual-tls/requirements.md` section 4. The CA, the trust store and the mTLS
   providers come from there.
4. Read `../80-platform-prerequisites/requirements.md` (the transport facts and the migration
   groups) and `../84-context-injection/requirements.md` (the entropy and IDs).
5. Read `LEARNINGS.md`.
6. Read `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md`.
7. The spec has one feature: `features/00-foundation-auth/feature.md`.
8. Follow `features/00-foundation-auth/start.md`.
9. Update `progress.md` and `LEARNINGS.md` after each task.

**Order:** spec 86, then spec 87, then spec 80, then this specification. This specification
unblocks 82 and 83.

**Owned crates:** `foundation_auth`, for `secret/`, the providers of `client_auth/` and the
interceptor. Do not edit `pki/`, `trust/` or `mtls/`: spec 87 owns them.

**Workflow:** Requirements → 86 and 87 baseline → Select feature → Feature start.md → Implement
→ Verify

---

_Created: 2026-11-14_
