---
feature: "Metadata Completion — Add missing descriptions, READMEs, and publish fields"
description: "Ensure all 38 crates have complete Cargo.toml metadata for crates.io publishing: descriptions, readme fields, repository URLs, and license confirmation"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-06-16
last_updated: 2026-06-16
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# Feature 01: Metadata Completion

## WHY: Problem Statement

crates.io requires every published crate to have complete metadata. Currently 4 crates were missing `description` fields (now added in feature 00), but none have `readme` fields pointing to per-crate READMEs. Without proper metadata, crates.io listings will be sparse and unhelpful to consumers.

## WHAT: Scope

For each of the 38 crates, ensure these `Cargo.toml` fields are present and correct:

| Field | Status | Notes |
|-------|--------|-------|
| `name` | ✅ All present | Confirmed available on crates.io |
| `version` | ✅ All present | Ranging from 0.0.1 to 0.1.0 |
| `edition` | ✅ Workspace-inherited | 2021 |
| `license` | ✅ Workspace-inherited | Apache-2.0 |
| `authors` | ✅ Workspace-inherited | EweStudios Consulting Limited |
| `repository` | ✅ Workspace-inherited | https://github.com/ewe-studios/ewe_platform |
| `description` | ✅ All present | 4 added in feature 00 |
| `readme` | ❌ Missing on all | Need per-crate README.md files |
| `documentation` | ❌ Missing on all | Optional (docs.rs auto-builds) |
| `categories` | ❌ Missing on some | crates.io taxonomy |

## HOW: Implementation Plan

### Tasks

1. **Audit all 38 Cargo.toml files** for completeness
   - Check each has: name, version, edition, license, authors, repository, description, readme
   - Flag any missing fields

2. **Create per-crate README.md files** (minimal, one paragraph each)
   - Each README should have: crate name, one-line description, link to repo, basic usage example
   - Place at `backends/<crate>/README.md` or `infrastructure/<crate>/README.md`

3. **Add `readme` field to each Cargo.toml**
   - Point to the README.md in the same directory: `readme = "README.md"`

4. **Add `categories` field** to each Cargo.toml
   - Use appropriate crates.io categories (e.g., "web-programming", "development-tools", etc.)

5. **Verify** — `cargo publish --dry-run` for a sample crate to confirm metadata is complete

### Verification

```bash
# Check all crates have description
for f in backends/*/Cargo.toml; do
  grep -q '^description = ' "$f" || echo "MISSING: $f"
done

# Dry-run publish a sample crate
cargo publish --dry-run -p foundation_errstacks
```

### Success Criteria

- All 38 crates have `description` and `readme` in Cargo.toml
- All 38 crates have a README.md file
- `cargo publish --dry-run` succeeds for at least one sample crate with no metadata warnings
