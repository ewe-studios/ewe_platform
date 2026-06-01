---
feature: "foundation_config"
description: "Move crates/config to backends/foundation_config, update workspace Cargo.toml, update all consumers"
status: "completed"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
---

# Feature: foundation_config

## Overview

Move `crates/config` (currently `ewe_config`) to `backends/foundation_config` with package name `foundation_config`. This is a minimal crate: two functions (`from_path`, `value_from_path`) for loading TOML config files. No async deps, fully `std::fs` based.

## Current state

- **Location:** `backends/foundation_config/`
- **Package name:** `foundation_config`
- **Dependencies:** `toml`, `derive_more`, `serde`, `serde_with`
- **Files:** `Cargo.toml`, `src/lib.rs` (64 lines)

## Completed Tasks

1. ✅ Moved `crates/config/` → `backends/foundation_config/`
2. ✅ Updated `Cargo.toml`: renamed package to `foundation_config`, updated keywords
3. ✅ Added to `workspace.dependencies` as `foundation_config = { path = "./backends/foundation_config", version = "0.1.0" }`
4. ✅ Found all crates that depend on `ewe_config` and updated to `foundation_config`
5. ✅ Ran `cargo check` to verify compilation — all green

## Cargo.toml target

```toml
[package]
name = "foundation_config"
version = "0.1.0"
description = "TOML-based configuration file loading utilities"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
keywords = ["config", "toml", "serde"]
```

---

_Created: 2026-06-01_
