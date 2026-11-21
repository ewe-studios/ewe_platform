---
workspace_name: "ewe_platform"
spec_directory: "specifications/81-foundation-auth"
feature_directory: "specifications/81-foundation-auth/features/00-foundation-auth"
this_file: "specifications/81-foundation-auth/features/00-foundation-auth/start.md"
created: 2026-11-14
---

# Start: Foundation Auth Feature

## Agent Workflow

1. Read `feature.md` (tasks T5, T7 and T11).
2. Read `auth.md` (the full design; 15 sections). Sections 5, 6, 7, 8.1 and 8.3 are pointers to
   spec 86 and spec 87.
3. Read `../../requirements.md` (what this library owns and what it does not).
4. Read `../../../86-auth-primitives/requirements.md` section 3. The trait and the result types
   come from there.
5. Read `../../../87-mutual-tls/requirements.md` section 4. The CA, the trust store, the
   verifiers and the mTLS providers come from there.
6. Read `../../../84-context-injection/requirements.md` section 3 and
   `../../../80-platform-prerequisites/requirements.md` (entropy, IDs, transport facts,
   migrations).
7. Read `../../LEARNINGS.md`.
8. Read `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md`.
9. Work on one task at a time, test first, tests in `backends/foundation_auth/tests/`.
10. Report, verify, commit; update `../../progress.md` and `../../LEARNINGS.md`.

**Boundary:** this feature adds the server credential modules. It does not change the existing
OAuth, OIDC, JWT and session modules, and it does not edit `pki/`, `trust/` or `mtls/` (spec
87 owns them). Key protection at rest stays in spec 83 feature 08.

**Done when:** a signed secret verifies with the public key only, the provider set includes the
spec 87 mTLS providers, and no secret or key appears in `Debug` output.

---

_Created: 2026-11-14_
_Revised: 2026-11-15 (primitives moved to spec 86; mTLS moved to spec 87)._
