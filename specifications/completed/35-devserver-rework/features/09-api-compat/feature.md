---
feature: "API Compatibility"
description: "Preserve public API surface: ProjectDefinition, ProxyRemoteConfig, ProxyType, Http1/2/3, VecStringExt — minimize consumer breakage"
status: "complete"
priority: "medium"
depends_on: ["01-crate-scaffolding", "02-task-operators", "06-native-proxy", "08-dev-service"]
estimated_effort: "small"
created: 2026-06-01
last_updated: "2026-06-15"
---

# Feature: API Compatibility

## Problem

Several consumers import from `ewe_devserver`:

| Consumer | Imports |
|----------|---------|
| `bin/platform/src/local/mod.rs` | `ewe_devserver::{types::{Http1, ProxyRemoteConfig}, HttpDevService, ProjectDefinition, ProxyType, VecStringExt}` |
| `examples/devbinary/src/main.rs` | Same as above |
| `templates/api-app/src/bin/dev/main.rs` | Same (template-generated) |
| `templates/web-app/src/bin/dev/main.rs` | Same |
| `examples/web/wello/Cargo.toml` | `ewe_devserver` dependency |
| `templates/*/Cargo.toml` | `ewe_devserver` dependency |

## Solution

Preserve the core data types with minimal changes. Only the **usage pattern** changes (async → valtron).

### Types to Preserve

```rust
// -- types.rs (preserve exactly)
pub struct ProxyRemoteConfig { pub addr: String, pub port: usize }
pub struct Tunnel { pub source: ProxyRemoteConfig, pub destination: ProxyRemoteConfig }
pub struct Http1 { pub source: ProxyRemoteConfig, pub destination: ProxyRemoteConfig, pub routes: Option<HyperFuncMap> }
pub struct Http2 { /* stub */ }
pub struct Http3 { /* stub */ }
pub enum ProxyType { Tunnel(Tunnel), Http1(Http1), Http2(Http2), Http3(Http3) }

// -- config.rs (preserve HyperFuncMap equivalent)
pub type RouteHandler = dyn Fn(&HttpRequest) -> HttpResponse + Send + Sync + 'static;
pub type RouteMap = HashMap<String, Arc<RouteHandler>>;

// -- types.rs
pub struct ProjectDefinition {
    pub proxy: ProxyType,
    pub target_directory: String,
    pub workspace_root: String,
    pub crate_name: String,
    pub skip_rust_checks: bool,
    pub stop_on_failure: bool,
    pub reload_directories: Vec<String>,
    pub build_directories: Vec<String>,
    pub build_arguments: Vec<String>,
    pub run_arguments: Vec<String>,
    pub wait_before_reload: Duration,
}

// -- vec_ext.rs (preserve exactly)
pub trait VecStringExt { fn to_vec_string(self) -> Vec<String>; }
impl VecStringExt for Vec<&str> { fn to_vec_string(self) -> Vec<String> { ... } }
```

### Types that Change

| Old | New | Reason |
|-----|-----|--------|
| `BoxedError = Box<dyn std::error::Error + ...>` | `ToolingError` enum | Consistent error handling |
| `JoinHandle<T> = tokio::task::JoinHandle` | `Entry` (valtron) | Valtron task tracking |
| `Result<T> = std::result::Result<T, BoxedError>` | `Result<T, ToolingError>` | Consistent errors |
| `HyperFuncMap` | `RouteMap` | Raw HTTP handlers, not hyper |
| `HyperRequest`, `HyperResponse`, etc. | `HttpRequest`, `HttpResponse` | Raw types, not hyper |

### Migration Path for Consumers

```rust
// Old consumer code:
use ewe_devserver::{
    types::{Http1, ProxyRemoteConfig},
    HttpDevService, ProjectDefinition, ProxyType,
};
use tokio::sync::broadcast;

let mut dev = HttpDevService::new(definition);
let (tx, rx) = broadcast::channel(1);
let handle = dev.start(rx).await?;
handle.await??;

// New consumer code:
use foundation_toolings::{
    types::{Http1, ProxyRemoteConfig},
    DevService, ProjectDefinition, ProxyType,
    run_dev_server,
};

// Option A: one-liner
run_dev_server(definition)?;

// Option B: manual valtron control
let dev = DevService::new(definition);
let mut engine = SingleThreadedEngine::new(Config::default())?;
dev.start(&engine)?;
engine.run()?;
```

### Task Breakdown

1. [ ] Define all public types in `types.rs` (preserve signatures)
2. [ ] Define `ToolingError` in `error.rs`
3. [ ] Define `RouteMap` and `RouteHandler` in `config.rs`
4. [ ] Re-export all public items from `lib.rs`
5. [ ] Write `run_dev_server()` convenience function
6. [ ] Document public API

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/types.rs` | Create — preserved public types |
| `backends/foundation_toolings/src/config.rs` | Create — RouteMap, RouteHandler |
| `backends/foundation_toolings/src/lib.rs` | Edit — re-export all public items |

---

_Created: 2026-06-01_
