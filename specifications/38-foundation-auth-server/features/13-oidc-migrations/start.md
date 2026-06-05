---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/13-oidc-migrations/start.md"
feature_name: "13-oidc-migrations"
created: 2026-06-05
---

# Start: Feature 13 — OIDC Migrations

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_db/src/core/schema/migrations.rs` (existing migration runner)
4. Read existing SQL files (002, 003, 006, 010, 011) for style reference
5. Create `016_create_oauth_clients.sql`
6. Create `017_create_authorization_codes.sql`
7. Create `018_create_refresh_tokens.sql`
8. Create `019_create_device_codes.sql`
9. Update `migrations.rs` to include 4 new entries
10. Update migration count in existing tests
11. Run `cargo check -p foundation_db 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
12. Run `cargo test -p foundation_db` to verify migration tests pass
13. Update `LEARNINGS.md`

_Created: 2026-06-05_