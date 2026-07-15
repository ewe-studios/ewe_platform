---
feature: "Migrate QEMU/UTM providers to associated-types Provider trait"
description: "Move QEMU/UTM from testbed's concrete Provider trait to the platform's associated-types Provider trait alongside Docker. Delete the testbed's Provider trait."
status: "in-progress"
priority: "high"
phase: 3
depends_on: ["03-provider-trait-integration", "28-testbed-migration"]
estimated_effort: "large"
created: 2026-07-15
---
# Feature 10: Platform Provider Migration

## Why

Decision 03 designed the platform `Provider` trait with associated types so Docker
(`Handle=ContainerHandle, Config=ContainerConfig`) and QEMU/UTM
(`Handle=VmHandle, Config=VmProfile`) can coexist. The testbed's concrete
`Provider` trait (QEMU-shaped, no associated types) is the old design. This
feature migrates QEMU/UTM to implement the new trait and deletes the old one.

## What changes

### foundation_deployment_platform

**`providers/mod.rs`** — Unified trait + dispatch:

```rust
/// ProviderId — all backends
pub enum ProviderId { Qemu, Utm, Docker }

/// The single Provider trait (from Decision 03)
pub trait Provider: Send + Sync {
    type Handle;
    type Config;
    type Error: Debug + Display;

    fn name(&self) -> &'static str;
    fn id(&self) -> ProviderId;
    fn launch(&self, config: &Self::Config) -> Result<Self::Handle, Self::Error>;
    fn stop(&self, handle: &Self::Handle) -> Result<(), Self::Error>;
    fn is_running(&self, handle: &Self::Handle) -> bool;
    fn resolved_ports(&self, handle: &Self::Handle) -> Result<ResolvedPorts, Self::Error>;
    fn host_health(&self) -> HostHealth;
}

/// PlatformHandle — enum dispatch over backends
pub enum PlatformHandle {
    Qemu(VmHandle),
    Utm(VmHandle),
    Docker(ContainerHandle),
}

/// default_provider() — Docker on Linux if available, else QEMU; UTM on macOS
pub fn default_provider() -> Result<Box<dyn Provider<Config = ???>>>;
```

**`providers/qemu/mod.rs`** — `QemuProvider: Provider<Handle=VmHandle, Config=VmProfile, Error=TestbedError>`

**`providers/utm/mod.rs`** — `UtmProvider: Provider<Handle=VmHandle, Config=VmProfile, Error=TestbedError>`

**`providers/docker/mod.rs`** — already implements the trait, keep as-is

**Consumer impact:** 21+ files that call `provider.launch(profile, mode)` become
`provider.launch(profile)` (DisplayMode moves to provider construction). All
`use crate::vms::providers::Provider` → `use foundation_deployment_platform::providers::Provider`.

### foundation_testbed

- Delete `src/vms/` — moved to platform
- Update imports to `foundation_deployment_platform::*`
- Keep `src/wasm/` and `src/bin/wasm-testbed.rs`

## Provider API mapping

| Old (testbed) | New (platform) |
|---|---|
| `launch(profile, mode)` | `launch(profile)` — mode set on provider: `QemuProvider::new().display(Headless)` |
| `stop(handle)` | `stop(handle)` |
| `is_running(handle)` | `is_running(handle)` |
| `resolved_ports(handle)` | `resolved_ports(handle)` |
| `monitor_command(handle, cmd)` | `QemuProvider::monitor_command(handle, cmd)` — concrete method, not on trait |
| `ensure_image(profile)` | `QemuProvider::ensure_image(profile)` — concrete method, not on trait |
| `host_health()` | `host_health()` |

## Verification

- `cargo check -p foundation_deployment_platform`
- `cargo check -p foundation_testbed`
- Existing tests pass
- `default_provider()` returns correct backend per OS
