---
feature: "CI runs clippy for foundation_deployment_platform with the vms feature on"
status: "pending"
lane: "B"
model: "haiku"
branch: "fix/io-51-60-lane-b"
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

- [ ] The `vms` clippy run is in CI and passes on GitHub; backlog §7 entry deleted.
