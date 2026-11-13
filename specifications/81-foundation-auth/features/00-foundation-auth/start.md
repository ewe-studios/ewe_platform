---
workspace_name: "ewe_platform"
spec_directory: "specifications/81-foundation-auth"
feature_directory: "specifications/81-foundation-auth/features/00-foundation-auth"
this_file: "specifications/81-foundation-auth/features/00-foundation-auth/start.md"
created: 2026-11-14
---

# Start: Foundation Auth Feature

## Agent Workflow

1. Read `feature.md` (tasks T5 and T7 to T11).
2. Read `auth.md` (the full design; 15 sections).
3. Read `../../requirements.md` (what this library owns and what stays out).
4. Read `../../../80-platform-prerequisites/requirements.md` and the Env feature (entropy, IDs,
   transport facts, migrations).
5. Read `../../LEARNINGS.md`.
6. Read `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md`.
7. Work on one task at a time, test first, tests in `backends/foundation_auth/tests/`.
8. Report, verify, commit; update `../../progress.md` and `../../LEARNINGS.md`.

**Boundary:** this feature adds the server credential modules. It does not change the existing
OAuth, OIDC, JWT and session modules. Key protection at rest stays in spec 65 feature 08.

**Done when:** the five modules in `requirements.md` section 4 exist, a signed secret verifies
with the public key only, and no secret or key appears in `Debug` output.

---

_Created: 2026-11-14_
