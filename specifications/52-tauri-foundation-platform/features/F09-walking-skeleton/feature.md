---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F09-walking-skeleton"
this_file: "specifications/52-tauri-foundation-platform/features/F09-walking-skeleton/feature.md"

status: pending
priority: critical
created: 2026-07-17

depends_on:
  - "F01-session-backbone"
  - "F02-route-handler"
  - "F03-ewe-protocol"
  - "F06-webview-stack"
  - "F07-cache-tiers"

tasks:
  completed: 0
  uncompleted: 12
  total: 12
  completion_percentage: 0%
---

# F09 — Walking skeleton

## Overview

Wire everything together end-to-end: boot the platform, create WebView,
register routes, handle navigation, render content. Exercise the full
9-step execution contract from [decision 02](../decisions/02-route-policy-model.md#execution-contract-what-happens-after-a-decision).

---

## Part A — Full 9-step trace

```
1. Navigation interception → Tauri on_navigation() fires → session constructs NavigationIntent
2. Route handler chain → resolve_route() iterates handlers → first match wins
3. Cache check → CachePolicy evaluated → serve cached or proceed
4. Presentation → stack manager pushes/pops/morphs based on Presentation variant
5. Backend query → RouteSource resolution (WebviewApp/IpcShell/RemoteServer)
6. Protocol selection → priority chain (decision → proto query → detect → default)
7. Content encoding → protocol.encode() → Content-Type header
8. View instantiation → WebView created or reused, content delivered
9. Post-render → navigation recorded, screenshot captured, event emitted
```

## Part B — Integration tests

```rust
#[platform_test]
async fn link_click_navigates_to_remote_route() { ... }
#[platform_test]
async fn handler_chain_first_match_wins() { ... }
#[platform_test]
async fn cache_first_serves_cached_content() { ... }
#[platform_test]
async fn push_presentation_creates_screenshot() { ... }
#[platform_test]
async fn full_navigation_cycle_app_remote_back() { ... }
```

## Verification
```bash
cargo test --package foundation_platform -- walking_skeleton
```
