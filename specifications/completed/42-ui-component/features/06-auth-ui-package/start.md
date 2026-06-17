---
workspace_name: "ewe_platform"
spec_directory: "specifications/42-ui-component"
this_file: "specifications/42-ui-component/features/06-auth-ui-package/start.md"
feature_name: "06-auth-ui-package"
created: 2026-06-05
updated: 2026-06-13
---

# Start: Feature 06 — Auth UI Package

## Workflow

1. Read `feature.md` (comprehensive spec — rauthy page audit complete)
2. Read `specifications/42-ui-component/requirements.md`
3. Read `.agents/skills/rust-clean-code/skill.md`
4. Read `specifications/38-foundation-auth-server/requirements.md` (server API spec)
5. Read feature 05 (headless catalog — F1–F8, M1–M8)
6. Read feature 00 (slot composition) and feature 01 (`<Show>`/`<For>`)
7. Read feature 03 (App bootstrap) and feature 04 (mount protocol)
8. Create `backends/foundation_auth_ui/` crate
9. Create `backends/foundation_auth_ui/Cargo.toml`
10. Implement pages: AuthLayout, AuthHome, LoginPage, RegisterPage,
    PasswordResetRequest, PasswordSetPage, LogoutPage, DeviceAuthPage,
    ProviderCallback, EmailConfirmPage, RevokePage, AccountPage, ErrorPage
11. Implement support modules: api.rs, types.rs, pow.rs, session.rs,
    webauthn.rs, tos.rs
12. Run `cargo check -p foundation_auth_ui 2>&1 | tee /tmp/cargo-check.log`
13. Update `LEARNINGS.md`

**Dependencies:** Requires features 00–05 (catalog + machinery) to be
completed first. This is the LAST feature in spec 42.

_Created: 2026-06-05 · Updated: 2026-06-13 (resolved TODOs, full rauthy audit)_
