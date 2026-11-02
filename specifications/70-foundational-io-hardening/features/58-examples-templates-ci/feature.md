---
feature: "examples/todo, the web-app template and the vms CI lint"
status: "pending"
lane: "B"
model: "haiku"
branch: "fix/io-51-60-lane-b"
crates: ["examples/todo", "bin/platform", "foundation_deployment_platform", "ci"]
catalogue_rows: [28]
---

# Feature 58: examples, templates and the vms lint

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

- **E1 examples/todo** (backlog §23): `src/lib.rs` holds only a dead `fn main`; ten
  dependencies are unused. Spec 70 feature 24 ported the todo example's server; check what is
  left. If the crate has no purpose beyond that server, make `lib.rs` hold the real shared
  code (or delete `lib.rs` and the `[lib]` target) and remove every unused dependency.
  Verify: `cargo check -p <todo package>` without warnings; its test still passes.
- **E2 web-app template** (backlog §24): `src/app.rs`, `src/config.rs` and the private
  re-exports in the template's `src/lib.rs` are unused in the generated project. Wire them:
  the dev binary deserializes config into `ServerConfig`; the app binary builds `App`. If
  that design is not obvious from the template, delete the unused scaffolding instead. Verify
  with the template build test from spec 70 feature 24 (`tests/templates_build.rs` or its
  equivalent): the generated project builds **without warnings**.
- **E3 vms lint** (backlog §7): add a clippy run with `--features vms` for
  `foundation_deployment_platform` to `ci/packages.toml`. Verify:
  `python3 scripts/ci/ci.py plan` shows it and it passes.

## Done when

- [ ] Three entries fixed and deleted. CI green.
