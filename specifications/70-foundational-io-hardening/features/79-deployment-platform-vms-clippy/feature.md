---
feature: "CI runs clippy for foundation_deployment_platform with the vms feature on"
status: "done"
lane: "B"
model: "haiku"
branch: "spec70/f59-f60-f79-lane-b-mechanical"
crates: ["foundation_deployment_platform", "ci/packages.toml"]
catalogue_rows: [102]
---

# Feature 79: vms clippy in CI (lowest priority, last in lane B)

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Kept from dropped feature 58 E3 by the owner (2026-11-04). Backlog §7.

## E1. Add the `vms` clippy run

CI never runs clippy for `foundation_deployment_platform` with `--features vms`, so lints in
that code are not seen.
- Add `--features vms` to the package's `clippy-runs` in `ci/packages.toml`.
- Run `cargo clippy -p foundation_deployment_platform --profile dev --all-targets --features vms -- -D warnings`
  locally and fix what it reports (fix the code; do not add `allow` attributes unless a lint is
  wrong for that line, and then say why in a comment).
- `python3 scripts/ci/ci.py plan` shows the new run.

## Done when

- [x] The `vms` clippy run is in CI and passes on GitHub; backlog §7 entry deleted.

## Result

- `ci/packages.toml`: `foundation_deployment_platform` now sets
  `clippy-runs = ["", "vms"]` and `clippy-deny-warnings = true`. CI therefore
  runs `cargo clippy --all-targets --no-deps -- -D warnings` for the default
  and the `vms` feature trees.
- `python3 scripts/ci/ci.py list` shows
  `clippy=enforce+deny ... runs=['', 'vms']`.
- `python3 scripts/ci/ci.py run foundation_deployment_platform --step clippy --profile dev`
  ran both planned commands with `-D warnings` and they finished clean.
- The literal local command without `--no-deps` re-lints path dependencies.
  It fails on `foundation_nostd/src/comp/fair_gate.rs` (five lints), which is
  lane A debt already recorded in backlog §4. CI passes `--no-deps`, so that
  entry still holds the dependency fix.
- Backlog §7 entry deleted.
