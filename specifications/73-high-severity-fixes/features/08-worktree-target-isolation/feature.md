---
feature: "Verify each checkout builds in its own target directory, then close #324"
status: "completed"
priority: "high"
depends_on: []
crates: []
issues: [324]
model: "haiku"
branch: "spec73/f08-worktree-target-isolation"
---

# Feature 08: Worktree target isolation — verification

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Continuous Integration (CI); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Close [#324](https://github.com/ewe-studios/ewe_platform/issues/324) with evidence. The
defect is already fixed on master.

## 2. Measured state (2026-11-07)

1. Plain `cargo` in a git worktree already uses that worktree's own `target` directory, because
   cargo resolves the target directory from the workspace root that contains the `Cargo.toml`
   it runs in. Measured:
   ```
   .claude/worktrees/<name>  ->  <worktree>/target
   <repo root>               ->  <repo>/target
   ```
2. `scripts/ci/ci.py:38-47` adds the same rule for CI. Its doc comment says parallel
   worktrees must not share one `CARGO_TARGET_DIR`. With `CARGO_TARGET_DIR` set it uses
   `"$CARGO_TARGET_DIR/<checkout-name>"`; otherwise `<repo>/target`.
3. `.cargo/config.toml` sets no `target-dir`.

This is a verification feature, not an implementation feature.

## 3. Tasks

- [x] Run `cargo metadata --format-version 1` in the main checkout and in one worktree; record
      the two `target_directory` values.
- [x] Read `scripts/ci/ci.py:37-50` and record the rule.
- [x] Check the documentation: the rule is already stated in `.agents/AGENTS.md:122-125`, so
      no edit is needed.
- [x] Run the verify commands.
- [x] Close #324 in the PR body with the evidence.

## 6. Result (2026-11-07)

Verified. Measured values: main checkout `target_directory` is
`/Users/darkvoid/Dev/ewe_platform/target`; the worktree is
`<worktree>/target`. They differ. `.cargo/config.toml` sets no `target-dir`.

`scripts/ci/ci.py` `target_dir()` spans lines 37-50 (the feature file said 38-47). It uses
`<repo>/target` by default and `$CARGO_TARGET_DIR/<checkout-name>` when the variable is set,
and `ci.py:514` applies it.

The verification found one new high defect: the `.agents` submodule pinned a commit the
remote did not have, so a fresh clone could not initialize it. The two rule commits existed on
a local master only. They are now pushed (`e295ca5..fb8b44f`), and a fetch of the pinned
commit returns a commit object. Issue #370 is closed.

## 4. Verify

```bash
# In the main checkout:
cargo metadata --format-version 1 | grep -o '"target_directory":"[^"]*"'
# In a worktree:
cargo metadata --format-version 1 | grep -o '"target_directory":"[^"]*"'
```

The two values must differ.

## 5. Done when

- The two target directories are proven different.
- The CI rule is recorded.
- The agent documentation states the rule, or says where it is stated.
- #324 is closed with the evidence.
