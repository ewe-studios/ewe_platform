# 28 — Testbed migration: vms/ from foundation_testbed → deployment_platform

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Move `vms/` directory (VM definitions, QEMU configs, cloud-init templates) from
`foundation_testbed` into `foundation_deployment_platform`. The platform crate
owns container and VM lifecycle; the testbed is a consumer that runs test
scenarios using the platform crate's primitives.

## Why

`foundation_testbed` was the original test infrastructure. Spec-53 created
`foundation_deployment_platform` as the canonical home for Docker container
lifecycle (bollard wrappers, `ContainerHandle`, `WaitFor`, networking).
The `vms/` directory (QEMU VM definitions) belongs in the same crate —
containers and VMs are both infrastructure primitives that the platform manages.

Having them in `foundation_testbed` creates the same layering inversion we
fixed with sshkit: the testbed depends on the platform, but the platform's VM
code lives in the testbed.

## What moves

```
foundation_testbed/vms/
├── qemu_config.rs     →  foundation_deployment_platform/src/vm/
├── cloud_init.rs      →  foundation_deployment_platform/src/vm/
├── windows/           →  foundation_deployment_platform/src/vm/windows/
└── linux/             →  foundation_deployment_platform/src/vm/linux/
```

`foundation_testbed` keeps its test scenario files — it just imports VM types
from the platform crate instead of defining them locally.

## Impact

- `foundation_testbed` Cargo.toml: add `foundation_deployment_platform` dep,
  remove local `vms/` module
- `foundation_deployment_platform` Cargo.toml: add `serde_yaml` (for cloud-init),
  add `vm` feature gate (optional — not every platform consumer needs VMs)
- All imports of `foundation_testbed::vms::*` → `foundation_deployment_platform::vm::*`

## Verification

1. `cargo check -p foundation_deployment_platform` — compiles with `vm` feature.
2. `cargo check -p foundation_testbed` — compiles after import updates.
3. Existing testbed tests pass (no functional change, just file moves).
