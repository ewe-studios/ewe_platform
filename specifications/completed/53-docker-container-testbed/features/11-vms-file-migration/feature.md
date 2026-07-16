---
feature: "Mechanical file migration: vms/ from testbed → platform"
description: "Move all vms/ source files from foundation_testbed/src/vms/ to foundation_deployment_platform/src/, fix crate::vms:: → crate::, wire up deps"
status: "in-progress"
priority: "high"
phase: 3
depends_on: ["10-platform-provider-migration"]
estimated_effort: "medium"
created: 2026-07-15
---
# Feature 11: Mechanical file migration (vms/ → platform)

## Why

Decision 28 and 03 say: move everything from `foundation_testbed/src/vms/` into
`foundation_deployment_platform`. This feature handles the mechanical part — file
moves, import fixes, dependency wiring. The provider trait migration (Feature 10)
does the logical restructuring.

## Consumer analysis

Every `crate::vms::` reference is **internal** to `vms/` files — no code outside
`vms/` imports from it directly. The testbed's `src/lib.rs` does `pub mod vms;`.
The testbed binary (`src/bin/testbed.rs`) uses `foundation_testbed::vms::cli`.

This means the migration is:
1. Move all 58 files from `testbed/src/vms/` → `platform/src/`
2. Global find-replace: `crate::vms::` → `crate::`
3. Delete `testbed/src/vms/` directory
4. Wire up `platform/src/lib.rs` with the modules
5. Update `testbed/src/bin/testbed.rs`: `foundation_testbed::vms::cli` → `foundation_deployment_platform::cli`

## File layout (target)

```
foundation_deployment_platform/src/
├── lib.rs              ← add module declarations
├── docker/             ← existing, unchanged
├── providers/
│   ├── mod.rs          ← merged Provider trait (Feature 10)
│   ├── docker/         ← existing DockerProvider
│   ├── qemu/           ← moved from testbed
│   ├── utm/            ← moved from testbed
│   └── http.rs         ← moved from testbed
├── ssh/                ← moved from testbed
├── winrm/              ← moved from testbed
├── bootstrap/          ← moved from testbed
├── qemu/               ← moved from testbed (QEMU utilities)
├── config.rs           ← moved from testbed (VmProfile, GuestOs)
├── doctor/             ← moved from testbed
├── artifacts.rs        ← moved from testbed
├── cli/                ← moved from testbed
├── state/              ← moved from testbed
├── runner/             ← moved from testbed
├── export/             ← moved from testbed
├── import/             ← moved from testbed
├── host_bootstrap/     ← moved from testbed
├── init/               ← moved from testbed
├── build/              ← moved from testbed
└── bin/platform.rs     ← moved from testbed bin/testbed.rs
```

## Cargo.toml changes

### platform crate gains

```toml
# VM infrastructure (optional, gated on vms feature)
ssh2 = { version = "0.9", features = ["vendored-openssl"], optional = true }
indicatif = { version = "0.17", optional = true }
tar = { version = "0.4", optional = true }
flate2 = { version = "1", optional = true }
xz2 = { version = "0.1", optional = true }
dirs = { version = "6", optional = true }
anyhow = { version = "1", optional = true }
image = { version = "0.25", optional = true }
libc = { version = "0.2", optional = true }
chrono = { version = "0.4", optional = true }
toml = { version = "0.8", optional = true }
fs2 = { version = "0.4", optional = true }
sha2 = { version = "0.10", optional = true }
whoami = { version = "2", optional = true }
clap = { version = "4", features = ["derive"], optional = true }
portpicker = { version = "0.1", optional = true }
foundation_sshkit = { path = "../foundation_sshkit", optional = true }

[features]
vms = [
    "dep:ssh2", "dep:indicatif", "dep:tar", "dep:flate2", "dep:xz2",
    "dep:dirs", "dep:anyhow", "dep:image", "dep:libc", "dep:chrono",
    "dep:toml", "dep:fs2", "dep:sha2", "dep:whoami", "dep:clap",
    "dep:portpicker", "dep:foundation_sshkit", "dep:foundation_netio",
]
```

### testbed crate loses

Drop `vms` feature and all its deps (ssh2, indicatif, tar, flate2, xz2, dirs,
anyhow, image, libc, chrono, toml, fs2, sha2, whoami). Add:

```toml
foundation_deployment_platform = { path = "../foundation_deployment_platform", features = ["vms"] }
```

## Verification

1. `cargo check -p foundation_deployment_platform` — compiles (without vms feature)
2. `cargo check -p foundation_deployment_platform --features vms` — compiles with vms
3. `cargo check -p foundation_testbed` — compiles as thin consumer
4. `cargo check -p foundation_testbed --features wasm` — wasm still compiles
5. `cargo test -p foundation_deployment_platform --lib` — unit tests pass
