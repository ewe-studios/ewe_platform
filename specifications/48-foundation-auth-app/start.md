---
workspace_name: "ewe_platform"
spec_directory: "specifications/48-foundation-auth-app"
this_file: "specifications/48-foundation-auth-app/start.md"
feature_name: "overview"
created: 2026-06-16
updated: 2026-06-16
---

# Start: foundation_auth_app

## Workflow

1. Read `requirements.md` (gap analysis + feature index)
2. Read `specifications/completed/38-foundation-auth-server/requirements.md` (baseline)
3. Read `specifications/42-ui-component/features/06-auth-ui-package/feature.md` (UI detail)
4. Read `.agents/skills/rust-clean-code/skill.md`
5. Implement F01–F09 in `foundation_auth/src/server/` (extend existing module)
6. Implement F10 (`foundation_auth_ui` — new crate in backends/)
7. Implement F11 (`foundation_auth_app` — new crate in apps/)
8. Run `cargo check` on all affected crates
9. Run `foundation_testbed` E2E tests
10. Update `LEARNINGS.md`

_Created: 2026-06-16_
