---
feature: "The templates pin the published `foundation_toolings` version"
status: "pending (CI proof)"
lane: "after 84"
model: "haiku"
branch: "spec70/f110-toolings-template-pin"
crates: ["bin/platform"]
catalogue_rows: []  # from docs/bug-backlog.md §34; not a §8 catalogue row
---

# Feature 110: stale `foundation_toolings` pin in the templates

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-05, feature 82)

The `web-app` and `api-app` templates pin

```toml
foundation_toolings = { version = "0.0.4" }
```

but the workspace crate `backends/foundation_toolings` is version 0.0.5. Feature 82 updated
the `foundation_http` pins to 0.0.5 and did not touch this pin, because that feature does not
bump toolings.

`bin/platform/tests/templates_build.rs` patches crates.io with the local crates. A pin that
is behind the local version makes cargo fetch the registry copy of `foundation_toolings`.
The generated project then builds against a published crate, not the workspace crate under
test. The check is weaker than it looks, and a published 0.0.4 may not exist for a fresh
crate.

## Design

1. Bump `foundation_toolings` to 0.0.5 in both template manifests:
   `bin/platform/templates/web-app/Cargo.toml.template` and
   `bin/platform/templates/api-app/Cargo.toml.template`.
2. Search every template and example manifest for the same class of stale pin:
   `grep -rn 'foundation_[a-z_]* = { version' bin/platform/templates examples`.
   For each crate whose workspace version is newer than the pin, bump it in the same change,
   or record it in `docs/bug-backlog.md` with the measured versions.
3. Do not change a crate version. This feature moves pins only.

## Tests

- `cargo test --profile uat -p ewe_platform --test templates_build` passes.
- The test's build output shows no fetch of a registry copy of `foundation_toolings`; a
  temporary check during the run can assert the patched path is used. If the test cannot see
  that, say so in the feature and keep the version assertion only.

## Verify

```bash
cargo fmt --all --check
cargo test --profile uat -p ewe_platform --test templates_build
```

## Done when

- [x] Both templates pin `foundation_toolings` 0.0.5.
- [x] `templates_build` passes.
- [x] The backlog §34 entry is deleted.
- [ ] CI green (pending the run on the pushed commit).

## Result (2026-11-05)

Both template manifests now pin `foundation_toolings = "0.0.5"`. The pin scan over
`bin/platform/templates` and `examples` found no other stale pin: `foundation_config` 0.1.2,
`foundation_core` 0.0.7, `foundation_http` 0.0.5 and `foundation_toolings` 0.0.5 all match
their workspace versions.

`cargo test --profile uat -p ewe_platform --test templates_build` passes (1 test, 84 s). Backlog
§34 is deleted.
