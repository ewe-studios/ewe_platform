---
feature: "Cleanup"
description: "Deprecate/remove crates/devserver, update workspace Cargo.toml, update templates/examples/bin/platform imports to use foundation_toolings"
status: "complete"
priority: "medium"
depends_on: ["08-dev-service", "09-api-compat", "11-integration"]
estimated_effort: "small"
created: 2026-06-01
last_updated: "2026-06-15"
---

# Feature: Cleanup

## Problem

After `foundation_toolings` is complete and verified, the old `crates/devserver` crate and all references to `ewe_devserver` must be updated to use the new crate.

## Solution

### 1. Update Workspace Cargo.toml

```toml
# Add:
foundation_toolings = { path = "./backends/foundation_toolings", version = "0.0.1" }

# Remove or deprecate:
# ewe_devserver = { path = "./crates/devserver", version = "0.0.2" }
```

### 2. Update Consumers

| File | Change |
|------|--------|
| `bin/platform/Cargo.toml` | Replace `ewe_devserver` → `foundation_toolings` |
| `bin/platform/src/local/mod.rs` | Update imports, replace async `HttpDevService::start()` → sync `run_dev_server()` |
| `examples/devbinary/Cargo.toml` | Replace `ewe_devserver` → `foundation_toolings` |
| `examples/devbinary/src/main.rs` | Update imports, remove `#[tokio::main]`, use sync API |
| `examples/web/wello/Cargo.toml` | Replace `ewe_devserver` → `foundation_toolings` |
| `templates/api-app/Cargo.toml` | Replace `ewe_devserver` → `foundation_toolings` |
| `templates/api-app/src/bin/dev/main.rs` | Update template code |
| `templates/web-app/Cargo.toml` | Replace `ewe_devserver` → `foundation_toolings` |
| `templates/web-app/src/bin/dev/main.rs` | Update template code |
| `templates/static-html-app/Cargo.toml` | Replace `ewe_devserver` → `foundation_toolings` |

### 3. Update Consumer Code (example: bin/platform/src/local/mod.rs)

```rust
// Old:
use ewe_devserver::{
    types::{Http1, ProxyRemoteConfig},
    HttpDevService, ProjectDefinition, ProxyType, VecStringExt,
};
use tokio::sync::broadcast;

pub async fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    // ... setup ...
    let mut dev_service = HttpDevService::new(definition);
    let (_cancel_sender, cancel_receiver) = broadcast::channel::<()>(1);
    let waiter = dev_service.start(cancel_receiver).await?;
    waiter.await??;
    Ok(())
}

// New:
use foundation_toolings::{
    types::{Http1, ProxyRemoteConfig},
    DevService, ProjectDefinition, ProxyType, VecStringExt,
    run_dev_server,
};

pub fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    // ... setup (same) ...
    run_dev_server(definition)?;
    Ok(())
}
```

Note: The `run()` function changes from `async fn` to `fn` — the CLI caller must be updated accordingly.

### 4. Remove crates/devserver

After all consumers are updated and verified:
- Delete `crates/devserver/` directory
- Remove from workspace `Cargo.toml` members (if explicitly listed)

### Task Breakdown

1. [ ] Update workspace `Cargo.toml` — add `foundation_toolings` dependency
2. [ ] Update `bin/platform/Cargo.toml` — swap dependency
3. [ ] Update `bin/platform/src/local/mod.rs` — imports + sync API
4. [ ] Update `bin/platform/src/sandbox/mod.rs` if it references devserver
5. [ ] Update `examples/devbinary/` — Cargo.toml + main.rs
6. [ ] Update `examples/web/wello/Cargo.toml`
7. [ ] Update `templates/api-app/` — Cargo.toml + dev/main.rs
8. [ ] Update `templates/web-app/` — Cargo.toml + dev/main.rs
9. [ ] Update `templates/static-html-app/Cargo.toml`
10. [ ] Update `mise.toml` if it references devserver
11. [ ] Delete `crates/devserver/` directory
12. [ ] Verify `cargo check --workspace` passes

## File Changes Summary

| File | Action |
|------|--------|
| Root `Cargo.toml` | Edit — add foundation_toolings, remove ewe_devserver |
| `bin/platform/Cargo.toml` | Edit — swap dependency |
| `bin/platform/src/local/mod.rs` | Edit — update imports + API |
| `examples/devbinary/Cargo.toml` | Edit — swap dependency |
| `examples/devbinary/src/main.rs` | Edit — update imports + sync API |
| `templates/*/Cargo.toml` | Edit — swap dependencies |
| `templates/*/src/bin/dev/main.rs` | Edit — update imports + sync API |
| `crates/devserver/` | Delete |

---

_Created: 2026-06-01_
