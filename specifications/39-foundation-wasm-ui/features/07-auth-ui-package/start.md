---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/07-auth-ui-package/start.md"
feature_name: "07-auth-ui-package"
created: 2026-06-05
---

# Start: Feature 07 — Auth UI Package

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `specifications/38-foundation-auth-server/requirements.md` (server API spec)
4. Read Feature 03 output (html! macro, TemplateResult)
5. Read Feature 05 output (Component trait)
6. Create `backends/foundation_auth_ui/` crate
7. Create `backends/foundation_auth_ui/Cargo.toml`
8. Implement LoginForm, RegisterForm, MfaChallenge, SessionStatus, AuthLayout
9. Run `cargo check -p foundation_auth_ui 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
10. Update `LEARNINGS.md`

**Dependencies:** Requires Features 01-06 (foundation_wasm_ui core) to be completed first.

_Created: 2026-06-05_